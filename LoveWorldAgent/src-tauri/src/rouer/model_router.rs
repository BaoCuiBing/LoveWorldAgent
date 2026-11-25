use axum::body::Bytes; // 模型设置路由：/api/models 读写 agent_config.json 的 llm 数组，api_key 脱敏且 **** 保留原值
use axum::extract::Path; // 路径参数提取
use axum::routing::{get, patch}; // GET/PATCH 方法
use axum::Router; // 路由器类型
use serde_json::{json, Map, Value}; // JSON 构造
use crate::agent::models; // 模型注册表
use crate::config; // 配置读写
use crate::state::{as_bool, body_object, fail, obj_str, ok, ApiResult, AppState}; // 请求解析与响应工具
pub fn router() -> Router<AppState> { // 注册模型路由
    Router::new() // 创建路由器
        .route("/api/models", get(list_models).post(create_model)) // 列举与新建
        .route("/api/models/{index}", axum::routing::put(update_model).delete(delete_model)) // 更新/删除
        .route("/api/models/{index}/enabled", patch(toggle_model)) // 启停切换
}
fn config_path() -> std::path::PathBuf { // 取配置文件路径
    if config::WRITABLE_AGENT_CONFIG_FILE.exists() { config::WRITABLE_AGENT_CONFIG_FILE.clone() } else { config::BUNDLED_AGENT_CONFIG_FILE.clone() } // 优先可写配置
}
fn mask(key: &str) -> String { // API Key 脱敏，保留前后各若干位
    let chars: Vec<char> = key.chars().collect(); // 字符数组
    if chars.is_empty() { return String::new(); } // 空 Key 返回空
    if chars.len() <= 8 { return "****".to_string(); } // 短 Key 全遮蔽
    format!("{}****{}", chars[..3].iter().collect::<String>(), chars[chars.len() - 4..].iter().collect::<String>()) // 保留前 3 后 4
}
fn public_item(item: &Value) -> Value { // 模型配置脱敏后对外展示（附带模型级参数可选项）
    let provider = item.get("provider").and_then(Value::as_str).unwrap_or_default().trim().to_lowercase(); // 服务商
    let model = item.get("model").and_then(Value::as_str).unwrap_or_default().trim().to_string(); // 模型名
    let display = models::display_of(item); // 显示名
    let raw_context = item.get("context").and_then(Value::as_str).unwrap_or(models::DEFAULT_CONTEXT); // 原始上下文档位
    let raw_thinking = item.get("thinking").and_then(Value::as_str).unwrap_or(models::DEFAULT_THINKING); // 原始思考档位
    json!({
        "provider": provider, // 服务商
        "base_url": item.get("base_url").and_then(Value::as_str).unwrap_or_default(), // API 地址
        "api_key": mask(item.get("api_key").and_then(Value::as_str).unwrap_or_default()), // 脱敏 Key
        "model": model, // 模型名
        "enabled": item.get("enabled").and_then(Value::as_bool).unwrap_or(true), // 启用状态
        "context": models::normalize_context(&provider, &model, raw_context), // 按模型归一化的上下文档位
        "thinking": models::normalize_thinking(&provider, &model, raw_thinking), // 按模型归一化的思考参数
        "context_options": models::context_options_of(&provider, &model), // 该模型可选上下文档位
        "thinking_title": models::thinking_title_of(&provider, &model), // 该模型思考参数标题
        "thinking_options": models::thinking_options_json(&provider, &model), // 该模型可选思考参数及中文标签
        "display": display, // 显示名
        "vision": models::vision_of(item), // 视觉能力
    })
}
fn read_llm(path: &std::path::Path) -> Vec<Value> { // 读取 llm 配置数组
    if !path.exists() { return Vec::new(); } // 不存在返回空
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new(); }; // 读取失败返回空
    let Ok(data) = serde_json::from_str::<Value>(&text) else { return Vec::new(); }; // 解析失败返回空
    models::normalize_llm(data.get("llm").unwrap_or(&Value::Null)) // 归一化 llm 段
}
fn write_llm(path: &std::path::Path, items: &[Value]) -> anyhow::Result<()> { // 写回 llm 配置数组
    let source = if path.exists() { Some(path.to_path_buf()) } else if config::BUNDLED_AGENT_CONFIG_FILE.exists() { Some(config::BUNDLED_AGENT_CONFIG_FILE.clone()) } else { None }; // 选取来源
    let mut data = source.as_deref().map(|target| config::read_json(Some(target))).unwrap_or_else(|| json!({})); // 读取配置
    if !data.is_object() { data = json!({}); } // 非对象重置
    data["llm"] = Value::Array(items.to_vec()); // 写入 llm 数组
    config::atomic_write_json(path, &data) // 原子写入
}
fn merge_item(existing: Option<&Value>, payload: &Value) -> Value { // 合并模型配置，**** 与空值保留原值
    let provider = payload.get("provider").and_then(Value::as_str).map(|text| text.trim().to_lowercase()).filter(|text| !text.is_empty()) // 提交的服务商
        .or_else(|| existing.and_then(|item| item.get("provider")).and_then(Value::as_str).map(|text| text.trim().to_lowercase())).unwrap_or_else(|| models::DEFAULT_PROVIDER.to_string()); // 缺省用原值或默认
    let mut key = obj_str(payload.as_object().unwrap(), "api_key").trim().to_string(); // 提交的 Key
    if key.contains("****") { if let Some(item) = existing { key = item.get("api_key").and_then(Value::as_str).unwrap_or_default().to_string(); } } // 遮蔽占位保留原值
    if key.is_empty() { // 空 Key 时按服务商匹配保留
        if let Some(item) = existing { // 存在原配置
            if provider == item.get("provider").and_then(Value::as_str).unwrap_or_default().to_lowercase() { key = item.get("api_key").and_then(Value::as_str).unwrap_or_default().to_string(); } // 同服务商则保留
        }
    }
    let base_url = obj_str(payload.as_object().unwrap(), "base_url").trim().to_string(); // 提交的地址
    let base_url = if base_url.is_empty() { existing.and_then(|item| item.get("base_url")).and_then(Value::as_str).map(str::to_string).filter(|text| !text.is_empty()).unwrap_or_default() } else { base_url }; // 空值保留原值
    let model = obj_str(payload.as_object().unwrap(), "model").trim().to_string(); // 提交的模型名
    let model = if model.is_empty() { existing.and_then(|item| item.get("model")).and_then(Value::as_str).map(str::to_string).unwrap_or_default() } else { model }; // 空值保留原值
    let raw_context = payload.get("context").and_then(Value::as_str).map(str::to_string).or_else(|| existing.and_then(|item| item.get("context")).and_then(Value::as_str).map(str::to_string)).unwrap_or_else(|| models::default_context_of(&provider, &model).to_string()); // 上下文档位
    let raw_thinking = payload.get("thinking").and_then(Value::as_str).map(|text| text.trim().to_lowercase()).or_else(|| existing.and_then(|item| item.get("thinking")).and_then(Value::as_str).map(|text| text.trim().to_lowercase())).unwrap_or_else(|| models::default_thinking_of(&provider, &model).to_string()); // 思考档位
    let context = models::normalize_context(&provider, &model, &raw_context); // 按模型校验归一化上下文
    let thinking = models::normalize_thinking(&provider, &model, &raw_thinking); // 按模型校验归一化思考
    let display = payload.get("display").and_then(Value::as_str).map(|text| text.trim().to_string()).filter(|text| !text.is_empty()) // 提交的显示名
        .or_else(|| existing.and_then(|item| item.get("display")).and_then(Value::as_str).map(|text| text.trim().to_string()).filter(|text| !text.is_empty())) // 保留原显示名
        .or_else(|| models::preset_display(&provider).map(str::to_string)).unwrap_or_else(|| provider.clone()); // 回退内置展示名或服务商键
    let vision = payload.get("vision").and_then(Value::as_bool).or_else(|| existing.and_then(|item| item.get("vision")).and_then(Value::as_bool)).unwrap_or_else(|| models::default_vision_of(&provider, &model)); // 视觉能力（新建时自动带出预设能力）
    let enabled = as_bool(payload.get("enabled"), existing.and_then(|item| item.get("enabled")).and_then(Value::as_bool).unwrap_or(true)); // 启用状态
    let mut merged = Map::new(); // 合并结果
    merged.insert("provider".to_string(), Value::String(provider)); // 服务商
    merged.insert("base_url".to_string(), Value::String(base_url)); // API 地址
    merged.insert("api_key".to_string(), Value::String(key)); // API Key
    merged.insert("model".to_string(), Value::String(model)); // 模型名
    merged.insert("enabled".to_string(), json!(enabled)); // 启用状态
    merged.insert("context".to_string(), Value::String(context)); // 按模型校验后的上下文档位
    merged.insert("thinking".to_string(), Value::String(thinking)); // 按模型校验后的思考参数
    merged.insert("display".to_string(), Value::String(display)); // 显示名
    merged.insert("vision".to_string(), json!(vision)); // 视觉能力
    Value::Object(merged) // 返回合并配置
}
fn list_payload(items: &[Value]) -> Value { // 构造模型列表响应
    json!({"total": items.len(), "list": items.iter().map(public_item).collect::<Vec<Value>>()}) // 脱敏后列表
}
async fn list_models() -> ApiResult { // 列出全部模型
    let path = config_path(); // 配置路径
    let items = read_llm(&path); // 读取模型
    let mut data = list_payload(&items); // 构造响应
    data["providers"] = Value::Array(models::providers_json(&items)); // 附加服务商聚合
    ok(data) // 返回结果
}
async fn create_model(body: Bytes) -> ApiResult { // 新建模型
    let payload = body_object(&body); // 解析请求体
    let map = payload.as_object().unwrap(); // 对象引用
    let provider = obj_str(map, "provider").trim().to_lowercase(); // 服务商
    let path = config_path(); // 配置路径
    let mut items = read_llm(&path); // 读取模型
    if !models::has(&items, &provider) { return fail(400, &format!("服务商必须是 {}", models::provider_keys(&items).join("/"))); } // 服务商校验
    if obj_str(map, "model").trim().is_empty() { return fail(400, "模型名不能为空"); } // 模型名校验
    items.push(merge_item(None, &payload)); // 追加新模型
    if let Err(err) = write_llm(&config::WRITABLE_AGENT_CONFIG_FILE, &items) { crate::utils::logger::error("model_router", &format!("保存模型失败: {err}")); return fail(500, "写入配置失败"); } // 落盘
    ok(list_payload(&items)) // 返回结果
}
async fn update_model(Path(index): Path<usize>, body: Bytes) -> ApiResult { // 更新模型
    let path = config_path(); // 配置路径
    let mut items = read_llm(&path); // 读取模型
    if index >= items.len() { return fail(404, "模型不存在"); } // 下标越界
    let payload = body_object(&body); // 解析请求体
    let provider = payload.get("provider").and_then(Value::as_str).map(|text| text.trim().to_lowercase()).filter(|text| !text.is_empty()) // 提交的服务商
        .or_else(|| items[index].get("provider").and_then(Value::as_str).map(|text| text.trim().to_lowercase())).unwrap_or_default(); // 缺省用原值
    if !models::has(&items, &provider) { return fail(400, &format!("服务商必须是 {}", models::provider_keys(&items).join("/"))); } // 服务商校验
    let existing = items[index].clone(); // 克隆原值
    items[index] = merge_item(Some(&existing), &payload); // 合并更新
    if let Err(err) = write_llm(&config::WRITABLE_AGENT_CONFIG_FILE, &items) { crate::utils::logger::error("model_router", &format!("保存模型失败: {err}")); return fail(500, "写入配置失败"); } // 落盘
    ok(list_payload(&items)) // 返回结果
}
async fn delete_model(Path(index): Path<usize>) -> ApiResult { // 删除模型
    let path = config_path(); // 配置路径
    let mut items = read_llm(&path); // 读取模型
    if index >= items.len() { return fail(404, "模型不存在"); } // 下标越界
    items.remove(index); // 删除指定项
    if let Err(err) = write_llm(&config::WRITABLE_AGENT_CONFIG_FILE, &items) { crate::utils::logger::error("model_router", &format!("删除模型失败: {err}")); return fail(500, "写入配置失败"); } // 落盘
    ok(list_payload(&items)) // 返回结果
}
async fn toggle_model(Path(index): Path<usize>, body: Bytes) -> ApiResult { // 切换模型启用状态
    let path = config_path(); // 配置路径
    let mut items = read_llm(&path); // 读取模型
    if index >= items.len() { return fail(404, "模型不存在"); } // 下标越界
    let payload = body_object(&body); // 解析请求体
    items[index]["enabled"] = json!(as_bool(payload.get("enabled"), true)); // 写入启用状态
    if let Err(err) = write_llm(&config::WRITABLE_AGENT_CONFIG_FILE, &items) { crate::utils::logger::error("model_router", &format!("切换模型状态失败: {err}")); return fail(500, "写入配置失败"); } // 落盘
    ok(list_payload(&items)) // 返回结果
}
