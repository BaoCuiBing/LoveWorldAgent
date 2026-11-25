use serde_json::{json, Value}; // 对话设置：config.json 的 chat 段容错读取与类型收敛，供路由层与工具装配复用
use crate::config; // 配置文件路径
use crate::state::as_bool; // 布尔值容错转换
fn volume_of(value: Option<&Value>) -> i64 { // 音量容错读取，钳制到 0-100
    match value { // 按配置值类型分派
        Some(Value::Number(number)) => number.as_i64().map(|raw| raw.clamp(0, 100)).unwrap_or(100), // 数字类型读取并钳制
        Some(Value::String(text)) => text.trim().parse::<i64>().map(|raw| raw.clamp(0, 100)).unwrap_or(100), // 字符串类型解析并钳制
        _ => 100, // 缺失或异常类型用默认音量
    }
}
pub fn normalize(raw: &Value) -> Value { // 把 chat 配置收敛成固定结构
    let data = raw.as_object(); // 顶层对象
    let notify = data.and_then(|map| map.get("notify")).and_then(Value::as_object); // 通知子对象
    json!({
        "ask": as_bool(data.and_then(|map| map.get("ask")), true), // 提问开关，缺省 true
        "todo": as_bool(data.and_then(|map| map.get("todo")), true), // 待办开关，缺省 true
        "notify": {
            "banner": as_bool(notify.and_then(|map| map.get("banner")), true), // 横幅通知开关
            "sound": as_bool(notify.and_then(|map| map.get("sound")), true), // 声音通知开关
            "volume": volume_of(notify.and_then(|map| map.get("volume"))), // 通知音量
        },
    })
}
pub fn read_chat() -> Value { // 依次从可写/随包配置读取 chat 段
    for path in [&*config::WRITABLE_CONFIG_FILE, &*config::BUNDLED_CONFIG_FILE] { // 优先可写配置，回退随包配置
        if !path.exists() { continue; } // 文件不存在跳过
        let Ok(text) = std::fs::read_to_string(path) else { continue; }; // 读取失败跳过
        let Ok(data) = serde_json::from_str::<Value>(&text) else { continue; }; // JSON 解析失败跳过
        return normalize(data.get("chat").unwrap_or(&Value::Null)); // 命中后收敛返回
    }
    normalize(&Value::Null) // 全部失败返回缺省配置
}
pub fn flags() -> Value { // 提取工具装配所需的提问/待办开关
    let chat = read_chat(); // 读取对话设置
    json!({"ask": chat.get("ask").cloned().unwrap_or(json!(true)), "todo": chat.get("todo").cloned().unwrap_or(json!(true))}) // 缺省均为 true
}
