use std::os::windows::process::CommandExt; // 标题栏主题：dark/light/system 映射到 Tauri window.set_theme，无法复刻的 DWM 细节用等价手段
use std::process::{Command, Stdio}; // 外部命令与管道
use std::time::Duration; // 延时重绘
use serde_json::{json, Value}; // JSON 构造
use tauri::{Manager, Theme}; // Tauri 窗口与主题
use crate::utils::logger; // 日志输出
pub const REPAINT_MODES: [&str; 7] = ["setting", "nca", "text", "policy", "invalidate", "hide", "minrestore"]; // 保留前端可选的重绘策略名
pub fn system_dark() -> bool { // 读取系统是否为深色主题
    let mut command = Command::new("reg"); // 用 reg 查询注册表
    command.args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize", "/v", "AppsUseLightTheme"]).stdout(Stdio::piped()).stderr(Stdio::null()).creation_flags(0x0800_0000); // 查询 AppsUseLightTheme，不弹窗
    match command.output() { // 执行命令
        Ok(output) => String::from_utf8_lossy(&output.stdout).contains("0x0"), // 值为 0 表示深色
        Err(err) => { // 查询失败
            logger::warn("titlebar", &format!("读取系统主题失败: {err}")); // 记录告警
            true // 缺省按深色
        }
    }
}
pub fn resolve_dark(theme: &str) -> bool { // 把主题设置解析为是否深色
    match theme.trim().to_lowercase().as_str() { // 按主题分派
        "light" => false, // 浅色
        "system" => system_dark(), // 跟随系统
        _ => true, // 其他按深色
    }
}
fn nudge_title(window: &tauri::WebviewWindow<tauri::Wry>) { // 通过微调标题触发标题栏重绘
    let title = window.title().unwrap_or_default(); // 取当前标题
    let _ = window.set_title(&format!("{title} ")); // 加空格触发重绘
    let target = window.clone(); // 克隆窗口供延时恢复
    std::thread::Builder::new().name("titlebar-nudge".to_string()).spawn(move || { // 后台恢复标题
        std::thread::sleep(Duration::from_millis(30)); // 短暂延时
        let _ = target.set_title(&title); // 恢复原标题
    }).ok(); // 启动失败忽略
}
pub fn apply(theme: &str, repaint: Option<&str>) -> Value { // 应用标题栏主题
    let dark = resolve_dark(theme); // 解析是否深色
    let handle = crate::state::shared().handle.lock().clone(); // 取应用句柄
    let Some(app) = handle else { return json!({"ok": false, "reason": "window-not-found", "theme": theme, "dark": dark}); }; // 无句柄
    let Some(window) = app.get_webview_window("main") else { return json!({"ok": false, "reason": "window-not-found", "theme": theme, "dark": dark}); }; // 无主窗口
    let target = match theme.trim().to_lowercase().as_str() { // 映射 Tauri 主题
        "light" => Some(Theme::Light), // 浅色
        "dark" => Some(Theme::Dark), // 深色
        _ => None, // 跟随系统
    };
    if let Err(err) = window.set_theme(target) { logger::warn("titlebar", &format!("设置窗口主题失败: {err}")); } // 设置主题
    let mode = repaint.map(str::to_string).unwrap_or_else(|| "setting".to_string()); // 重绘策略缺省 setting
    if mode == "text" { nudge_title(&window); } // text 模式通过标题微调重绘
    let hwnd = window.hwnd().map(|item| item.0 as isize).unwrap_or(0); // 取窗口句柄
    logger::info("titlebar", &format!("标题栏配色: theme={theme} dark={dark} repaint={mode} hwnd={hwnd}")); // 记录
    json!({"ok": true, "theme": theme, "dark": dark, "hwnd": hwnd, "repaint": mode}) // 返回结果
}
