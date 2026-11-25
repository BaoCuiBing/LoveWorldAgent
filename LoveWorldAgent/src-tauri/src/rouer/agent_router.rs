use axum::body::Bytes; // 自定义智能体路由：/api/agents CRUD 与 /api/agent/settings 运行参数读写
use axum::extract::Path; // 路径参数提取
use axum::routing::{get, patch}; // GET/PATCH 方法
use axum::Router; // 路由器类型
use serde_json::{json, Map, Value}; // JSON 构造
use crate::agent::agents_repo; // 智能体仓库
use crate::config; // 配置读写
use crate::state::{as_bool, body_object, fail, fail_exc, json_str, ok, ApiResult, AppState}; // 请求解析与响应工具
pub fn router() -> Router<AppState> { // 注册智能体路由
    Router::new() // 创建路由器
        .route("/api/agents", get(list_agents).post(create_agent)) // 列举与新建
        .route("/api/agents/{agent_id}", get(read_agent).put(update_agent).delete(remove_agent)) // 读取/更新/删除
        .route("/api/agents/{agent_id}/enabled", patch(toggle_agent)) // 启停切换
        .route("/api/agent/settings", get(get_agent_settings).put(save_agent_settings)) // 运行参数读写
}
fn agent_path() -> std::path::PathBuf { // 取智能体配置文件路径
    if config::WRITABLE_AGENT_CONFIG_FILE.exists() { config::WRITABLE_AGENT_CONFIG_FILE.clone() } else { config::BUNDLED_AGENT_CONFIG_FILE.clone() } // 优先可写配置
}
fn agent_section(data: &Value) -> Value { // 提取 agent 配置段并补默认值
    let section = data.get("agent").and_then(Value::as_object); // agent 子对象
    json!({
        "enabled": section.and_then(|map| map.get("enabled")).and_then(Value::as_bool).unwrap_or(true), // 缺省启用
        "model": section.and_then(|map| map.get("model")).and_then(Value::as_str).unwrap_or_default(), // 模型名
        "max_turns": section.and_then(|map| map.get("max_turns")).and_then(Value::as_i64).unwrap_or(500), // 缺省 500 轮
        "timeout_seconds": section.and_then(|map| map.get("timeout_seconds")).and_then(Value::as_i64).unwrap_or(180), // 缺省 180 秒
        "command_timeout_seconds": section.and_then(|map| map.get("command_timeout_seconds")).and_then(Value::as_i64).unwrap_or(60), // 缺省 60 秒
    })
}
fn write_agent(section: &Value) -> anyhow::Result<()> { // 写回 agent 配置段
    let source = agent_path(); // 配置文件路径
    let mut data = if source.exists() { config::read_json(Some(source.as_path())) } else { json!({}) }; // 读取现有配置
    if !data.is_object() { data = json!({}); } // 非对象重置
    let mut current = data.get("agent").and_then(Value::as_object).cloned().unwrap_or_default(); // 取现有 agent 段
    if let Some(entries) = section.as_object() { // 遍历更新字段
        for (key, value) in entries { current.insert(key.clone(), value.clone()); } // 逐字段覆盖
    }
    data["agent"] = Value::Object(current); // 写回 agent 段
    config::atomic_write_json(&config::WRITABLE_AGENT_CONFIG_FILE, &data) // 原子写入
}
async fn list_agents() -> ApiResult { // 列出所有智能体
    let items = agents_repo::list_agents(); // 列举
    ok(json!({"dir": agents_repo::root_dir().to_string_lossy(), "total": items.len(), "list": items})) // 返回目录与列表
}
async fn read_agent(Path(agent_id): Path<String>) -> ApiResult { // 读取单个智能体
    match agents_repo::read_agent(&agent_id) { // 读取
        Ok(data) => ok(data), // 成功返回
        Err(err) => fail_exc(&err.to_string()), // 失败返回错误
    }
}
async fn create_agent(body: Bytes) -> ApiResult { // 新建智能体
    let payload = body_object(&body); // 解析请求体
    let text = |key: &str| json_str(payload.get(key)); // 取字符串字段闭包
    let tools: Vec<Value> = payload.get("tools").and_then(Value::as_array).cloned().unwrap_or_default(); // 工具白名单
    match agents_repo::save_agent(&text("id"), &text("name"), &text("when"), as_bool(payload.get("callable"), true), &tools, &text("body"), as_bool(payload.get("enabled"), true)) { // 保存智能体
        Ok(data) => ok(data), // 成功返回
        Err(err) => fail_exc(&err.to_string()), // 失败返回错误
    }
}
async fn update_agent(Path(agent_id): Path<String>, body: Bytes) -> ApiResult { // 更新智能体，未提交字段保留现值
    let payload = body_object(&body); // 解析请求体
    let current = match agents_repo::read_agent(&agent_id) { Ok(data) => data, Err(err) => return fail_exc(&err.to_string()) }; // 读取现值
    let text = |key: &str, field: &str| -> String { // 取值闭包，缺省用现值
        payload.get(key).map(|value| json_str(Some(value))).unwrap_or_else(|| current.get(field).and_then(Value::as_str).unwrap_or_default().to_string()) // 提交值优先
    };
    let tools: Vec<Value> = payload.get("tools").and_then(Value::as_array).cloned().unwrap_or_else(|| current.get("tools").and_then(Value::as_array).cloned().unwrap_or_default()); // 工具白名单缺省用现值
    let callable = as_bool(payload.get("callable"), current.get("callable").and_then(Value::as_bool).unwrap_or(true)); // 可派发标志
    let enabled = as_bool(payload.get("enabled"), current.get("enabled").and_then(Value::as_bool).unwrap_or(true)); // 启用标志
    match agents_repo::save_agent(&agent_id, &text("name", "name"), &text("when", "when"), callable, &tools, &text("body", "body"), enabled) { // 覆盖保存
        Ok(data) => ok(data), // 成功返回
        Err(err) => fail_exc(&err.to_string()), // 失败返回错误
    }
}
async fn remove_agent(Path(agent_id): Path<String>) -> ApiResult { // 删除智能体
    match agents_repo::delete_agent(&agent_id) { // 删除
        Ok(data) => ok(data), // 成功返回
        Err(err) => fail_exc(&err.to_string()), // 失败返回错误
    }
}
async fn toggle_agent(Path(agent_id): Path<String>, body: Bytes) -> ApiResult { // 切换智能体启用状态（内置子智能体随内置工具开关联动，禁止手动修改）
    if agents_repo::is_builtin(&agent_id) { return fail(400, &format!("内置子智能体「{agent_id}」随对应内置工具开关联动，不能手动修改")); } // 内置项拦截
    let payload = body_object(&body); // 解析请求体
    match agents_repo::set_enabled(&agent_id, as_bool(payload.get("enabled"), true)) { // 写入启用状态
        Ok(data) => ok(data), // 成功返回
        Err(err) => fail_exc(&err.to_string()), // 失败返回错误
    }
}
async fn get_agent_settings() -> ApiResult { // 读取智能体运行参数
    ok(agent_section(&config::load_agent_config())) // 返回 agent 配置段
}
async fn save_agent_settings(body: Bytes) -> ApiResult { // 保存智能体运行参数（带范围校验）
    let payload = body_object(&body); // 解析请求体
    let map = payload.as_object().unwrap(); // 对象引用
    let current = agent_section(&config::load_agent_config()); // 读取现值
    let mut updates = Map::new(); // 待更新字段
    for (key, bounds) in [("max_turns", (1i64, 500i64)), ("timeout_seconds", (10, 600)), ("command_timeout_seconds", (5, 600))] { // 各参数合法范围
        if !map.contains_key(key) { continue; } // 未提交则跳过
        let raw = payload.get(key); // 原始值
        let value = match raw { // 数字或数字字符串
            Some(Value::Number(number)) => number.as_i64(), // 数字
            Some(Value::String(text)) => text.trim().parse::<i64>().ok(), // 字符串解析
            _ => None, // 其他类型无效
        };
        let Some(value) = value else { return fail(400, &format!("{key} 必须是整数")); }; // 非整数报错
        if value < bounds.0 || value > bounds.1 { return fail(400, &format!("{key} 需在 {}~{} 之间", bounds.0, bounds.1)); } // 越界报错
        updates.insert(key.to_string(), json!(value)); // 写入更新
    }
    if updates.is_empty() { return fail(400, "没有可保存的参数"); } // 无有效参数报错
    let mut section = current.as_object().cloned().unwrap_or_default(); // 以现值为底
    for (key, value) in &updates { section.insert(key.clone(), value.clone()); } // 合并更新
    let section = Value::Object(section); // 转为对象
    if let Err(err) = write_agent(&section) { crate::utils::logger::error("agent_router", &format!("保存智能体运行参数失败: {err}")); return fail(500, "写入配置失败"); } // 落盘，失败报错
    ok(section) // 返回最终配置
}
