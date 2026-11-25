use std::path::Path; // adb 路径
use std::thread; // 双击间隔
use std::time::Duration; // 超时
use base64::Engine; // 剪贴板 base64
use crate::mobile::adb::{self, ToolError, ToolResult}; // adb 与错误
fn pair(value: i64) -> String { value.to_string() } // 坐标转文本
pub fn tap(adb: &Path, serial: &str, x: i64, y: i64) -> ToolResult<()> { // 单击
    let xs = pair(x); // X
    let ys = pair(y); // Y
    adb::shell(adb, serial, &["input", "tap", &xs, &ys], Duration::from_secs(15))?; // 执行
    Ok(()) // 完成
}
pub fn double_tap(adb: &Path, serial: &str, x: i64, y: i64) -> ToolResult<()> { // 双击
    tap(adb, serial, x, y)?; // 第一次
    thread::sleep(Duration::from_millis(80)); // 双击间隔
    tap(adb, serial, x, y) // 第二次
}
pub fn long_press(adb: &Path, serial: &str, x: i64, y: i64, duration_ms: u64) -> ToolResult<()> { // 长按
    let xs = pair(x); // X
    let ys = pair(y); // Y
    let ms = duration_ms.max(400).to_string(); // 时长
    adb::shell(adb, serial, &["input", "swipe", &xs, &ys, &xs, &ys, &ms], Duration::from_secs(20))?; // 原地滑动
    Ok(()) // 完成
}
pub fn swipe(adb: &Path, serial: &str, from: (i64, i64), to: (i64, i64), duration_ms: u64) -> ToolResult<()> { // 滑动
    let x1 = pair(from.0); // 起点 X
    let y1 = pair(from.1); // 起点 Y
    let x2 = pair(to.0); // 终点 X
    let y2 = pair(to.1); // 终点 Y
    let ms = duration_ms.max(100).to_string(); // 时长
    adb::shell(adb, serial, &["input", "swipe", &x1, &y1, &x2, &y2, &ms], Duration::from_secs(25))?; // 执行
    Ok(()) // 完成
}
fn escape_text(text: &str) -> String { // ASCII 文本转义
    let mut out = String::new(); // 结果
    for ch in text.chars() { // 逐字符
        match ch { // 转义规则
            ' ' => out.push_str("%s"), // 空格
            '&' | '<' | '>' | '|' | ';' | '(' | ')' | '$' | '`' | '"' | '\'' | '\\' => { out.push('\\'); out.push(ch); } // shell 元字符
            _ => out.push(ch), // 普通字符
        }
    }
    out // 返回
}
fn clipboard_set(adb: &Path, serial: &str, text: &str) -> ToolResult<()> { // 通过广播写剪贴板（best-effort）
    let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes()); // base64
    let attempts: [&[&str]; 2] = [ // 两种常见广播
        &["am", "broadcast", "-a", "clipper.set", "-e", "text", text], // clipper
        &["am", "broadcast", "-a", "ADB_INPUT_TEXT", "--es", "msg", text], // ADB Keyboard
    ];
    for args in attempts { // 逐个尝试
        if let Ok(output) = adb::shell(adb, serial, args, Duration::from_secs(20)) { // 执行广播
            if output.contains("Broadcast completed") { // 广播成功
                adb::shell(adb, serial, &["input", "keyevent", "279"], Duration::from_secs(15))?; // KEYCODE_PASTE
                return Ok(()); // 完成
            }
        }
    }
    let _ = encoded; // 保留编码结果供后续扩展
    Err(ToolError::new("UNSUPPORTED_INPUT", format!("设备不支持 Unicode 输入：{text}"))) // 失败
}
pub fn text(adb: &Path, serial: &str, value: &str) -> ToolResult<()> { // 输入文本
    if value.trim().is_empty() { return Ok(()); } // 空文本直接返回
    if value.is_ascii() { // ASCII 走 input text
        let escaped = escape_text(value); // 转义
        adb::shell(adb, serial, &["input", "text", &escaped], Duration::from_secs(20))?; // 执行
        return Ok(()); // 完成
    }
    clipboard_set(adb, serial, value) // 非 ASCII 走剪贴板
}
pub fn key(adb: &Path, serial: &str, name: &str) -> ToolResult<()> { // 发送按键
    let code = match name.trim().to_lowercase().as_str() { // 语义按钮映射
        "home" => "KEYCODE_HOME", // 主页
        "back" => "KEYCODE_BACK", // 返回
        "enter" => "KEYCODE_ENTER", // 回车
        "power" => "KEYCODE_POWER", // 电源
        "menu" => "KEYCODE_MENU", // 菜单
        "recent" | "recent_apps" => "KEYCODE_APP_SWITCH", // 最近任务
        "volume_up" => "KEYCODE_VOLUME_UP", // 音量加
        "volume_down" => "KEYCODE_VOLUME_DOWN", // 音量减
        "delete" => "KEYCODE_DEL", // 删除
        "tab" => "KEYCODE_TAB", // Tab
        "escape" => "KEYCODE_ESCAPE", // Esc
        other if other.starts_with("keycode_") => return adb::shell(adb, serial, &["input", "keyevent", &other.to_uppercase()], Duration::from_secs(15)).map(|_| ()), // 原样 keycode
        other => return adb::shell(adb, serial, &["input", "keyevent", other], Duration::from_secs(15)).map(|_| ()), // 其他数字/名称原样
    };
    adb::shell(adb, serial, &["input", "keyevent", code], Duration::from_secs(15))?; // 执行
    Ok(()) // 完成
}
pub fn open_url(adb: &Path, serial: &str, url: &str) -> ToolResult<()> { // 打开网址
    adb::shell(adb, serial, &["am", "start", "-a", "android.intent.action.VIEW", "-d", url], Duration::from_secs(20))?; // 执行
    Ok(()) // 完成
}
pub fn clipboard(adb: &Path, serial: &str, action: &str, value: Option<&str>) -> ToolResult<String> { // 剪贴板读写（best-effort）
    match action { // 分派
        "set" => { let text = value.unwrap_or_default(); clipboard_set(adb, serial, text)?; Ok("已写入剪贴板".to_string()) } // 写
        "get" => { // 读
            let output = adb::shell(adb, serial, &["cmd", "clipboard", "get"], Duration::from_secs(15)).map_err(|err| ToolError::new("UNSUPPORTED_INPUT", err.text()))?; // 尝试系统命令
            if output.trim().is_empty() { Err(ToolError::new("UNSUPPORTED_INPUT", "设备不支持读取剪贴板")) } else { Ok(output.trim().to_string()) } // 返回
        }
        _ => Err(ToolError::new("INVALID_ARGUMENT", "action 仅支持 get/set")), // 参数错误
    }
}