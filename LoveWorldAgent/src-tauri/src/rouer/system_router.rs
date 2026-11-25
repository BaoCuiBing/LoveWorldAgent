use axum::body::Bytes; // 系统设置路由：/api/system/theme 与 /api/system/layout 写入 config.json 的 ui 段
use axum::routing::get; // GET 方法
use axum::Router; // 路由器类型
use serde_json::{json, Map, Value}; // JSON 构造
use crate::config; // 配置读写
use crate::state::{body_object, fail, ok, ApiResult, AppState}; // 请求解析与响应工具
pub const THEMES: [&str; 3] = ["dark", "light", "system"]; // 合法主题
pub const LAYOUT_TABS: [&str; 5] = ["", "todo", "browser", "file", "terminal"]; // 合法右栏标签
pub fn router() -> Router<AppState> { // 注册系统设置路由
    Router::new() // 创建路由器
        .route("/api/system/theme", get(get_theme).put(save_theme)) // 主题读写
        .route("/api/system/layout", get(get_layout).put(save_layout)) // 布局读写
}
fn config_path() -> std::path::PathBuf { // 取配置文件路径
    if config::WRITABLE_CONFIG_FILE.exists() { config::WRITABLE_CONFIG_FILE.clone() } else { config::BUNDLED_CONFIG_FILE.clone() } // 优先可写配置
}
fn read_ui() -> Value { // 读取 ui 配置段
    let path = config_path(); // 配置路径
    if !path.exists() { return json!({}); } // 不存在返回空
    let Ok(text) = std::fs::read_to_string(path) else { return json!({}); }; // 读取失败返回空
    let Ok(data) = serde_json::from_str::<Value>(&text) else { return json!({}); }; // 解析失败返回空
    data.get("ui").and_then(Value::as_object).map(|map| Value::Object(map.clone())).unwrap_or_else(|| json!({})) // 取 ui 子对象
}
fn read_theme() -> String { // 读取当前主题
    let ui = read_ui(); // ui 配置
    let theme = ui.get("theme").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| config::get_str(&config::CONFIG, &["ui", "theme"], "dark")); // 取主题，缺省 dark
    if THEMES.contains(&theme.as_str()) { theme } else { "dark".to_string() } // 非法值回退 dark
}
fn update_ui(patch: &Value) -> anyhow::Result<()> { // 合并写入 ui 配置段
    let path = config::WRITABLE_CONFIG_FILE.clone(); // 可写配置路径
    let source = if path.exists() { Some(path.clone()) } else if config::BUNDLED_CONFIG_FILE.exists() { Some(config::BUNDLED_CONFIG_FILE.clone()) } else { None }; // 选取来源
    let mut data = source.as_deref().map(|target| config::read_json(Some(target))).unwrap_or_else(|| json!({})); // 读取配置
    if !data.is_object() { data = json!({}); } // 非对象重置
    let mut ui = data.get("ui").and_then(Value::as_object).cloned().unwrap_or_default(); // 取现有 ui 段
    if let Some(entries) = patch.as_object() { // 遍历补丁
        for (key, value) in entries { ui.insert(key.clone(), value.clone()); } // 逐字段覆盖
    }
    data["ui"] = Value::Object(ui); // 写回 ui 段
    config::atomic_write_json(&path, &data) // 原子写入
}
fn width_of(data: &Map<String, Value>, key: &str) -> Option<i64> { // 读取宽度并钳制到 0-10000
    match data.get(key) { // 按值类型分派
        None | Some(Value::Null) => None, // 缺失视为无
        Some(Value::Number(number)) => number.as_f64().map(|value| (value as i64).clamp(0, 10000)), // 数字钳制
        Some(Value::String(text)) => text.trim().parse::<i64>().ok().map(|value| value.clamp(0, 10000)), // 字符串解析后钳制
        _ => None, // 其他类型无效
    }
}
fn normalize_layout(value: Option<&Value>) -> Value { // 归一化布局配置
    let empty = Map::new(); // 空对象
    let data = value.and_then(Value::as_object).unwrap_or(&empty); // 取布局对象
    let tab = data.get("rightTab").and_then(Value::as_str).unwrap_or_default().to_string(); // 右栏标签
    json!({
        "remember": data.get("remember") != Some(&Value::Bool(false)), // 缺省记住布局
        "sideWidth": width_of(data, "sideWidth"), // 侧栏宽度
        "rightWidth": width_of(data, "rightWidth"), // 右栏宽度
        "rightOpen": data.get("rightOpen") == Some(&Value::Bool(true)), // 缺省关闭右栏
        "rightTab": if LAYOUT_TABS.contains(&tab.as_str()) { tab } else { String::new() }, // 非法标签清空
    })
}
async fn get_theme() -> ApiResult { // 读取主题
    ok(json!({"theme": read_theme()})) // 返回主题
}
async fn save_theme(body: Bytes) -> ApiResult { // 保存主题
    let payload = body_object(&body); // 解析请求体
    let theme = payload.get("theme").and_then(Value::as_str).unwrap_or_default().trim().to_lowercase(); // 主题值
    if !THEMES.contains(&theme.as_str()) { return fail(400, "主题必须是 dark/light/system"); } // 合法性校验
    if let Err(err) = update_ui(&json!({"theme": theme.clone()})) { crate::utils::logger::error("system_router", &format!("保存主题失败: {err}")); return fail(500, "写入配置失败"); } // 落盘
    ok(json!({"theme": theme})) // 返回结果
}
async fn get_layout() -> ApiResult { // 读取布局
    let ui = read_ui(); // ui 配置
    ok(normalize_layout(ui.get("layout"))) // 返回归一化布局
}
async fn save_layout(body: Bytes) -> ApiResult { // 保存布局，未提交字段保留现值
    let payload = body_object(&body); // 解析请求体
    let current = normalize_layout(read_ui().get("layout")); // 读取现值
    let mut merged = current.as_object().cloned().unwrap_or_default(); // 以现值为底
    if let Some(entries) = payload.as_object() { // 遍历提交字段
        for (key, value) in entries { merged.insert(key.clone(), value.clone()); } // 逐字段覆盖
    }
    let layout = normalize_layout(Some(&Value::Object(merged))); // 归一化合并结果
    if let Err(err) = update_ui(&json!({"layout": layout.clone()})) { crate::utils::logger::error("system_router", &format!("保存窗口布局失败: {err}")); return fail(500, "写入配置失败"); } // 落盘
    ok(layout) // 返回布局
}
