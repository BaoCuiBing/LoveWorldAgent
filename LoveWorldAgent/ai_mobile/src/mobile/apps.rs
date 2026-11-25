use serde_json::{json, Value}; // JSON 结果
use std::path::Path; // adb 路径
use std::time::Duration; // 超时
use crate::mobile::adb::{self, ToolError, ToolResult}; // adb 与错误
pub fn list_apps(adb: &Path, serial: &str, include_system: bool) -> ToolResult<Value> { // 列出应用包名
    let args: &[&str] = if include_system { &["pm", "list", "packages"] } else { &["pm", "list", "packages", "-3"] }; // 是否含系统应用
    let text = adb::shell(adb, serial, args, Duration::from_secs(30))?; // 执行
    let mut packages: Vec<String> = text.lines().filter_map(|line| line.trim().strip_prefix("package:").map(|name| name.trim().to_string())).filter(|name| !name.is_empty()).collect(); // 解析
    packages.sort(); // 稳定排序
    Ok(json!({"packages": packages, "total": packages.len(), "includeSystem": include_system})) // 返回
}
fn package_of_component(text: &str) -> Option<String> { // 从组件名提取包名
    let token = text.split_whitespace().find(|item| item.contains('/'))?; // com.pkg/.Activity
    let (package, _) = token.split_once('/')?; // 拆分
    Some(package.trim().to_string()) // 返回包名
}
pub fn foreground(adb: &Path, serial: &str) -> ToolResult<Value> { // 当前前台应用
    let text = adb::shell(adb, serial, &["dumpsys", "activity", "activities"], Duration::from_secs(20))?; // 查询活动栈
    let mut component = String::new(); // 组件
    for line in text.lines() { // 逐行
        let line = line.trim(); // 去空白
        if line.starts_with("mResumedActivity") || line.starts_with("topResumedActivity") || line.starts_with("ResumedActivity") { // 前台标记
            if let Some((_, rest)) = line.split_once(':') { if let Some(found) = rest.split_whitespace().find(|item| item.contains('/')) { component = found.to_string(); break; } } // 取组件
        }
    }
    let package = package_of_component(&component); // 包名
    Ok(json!({"component": component, "package": package})) // 返回
}
pub fn launch(adb: &Path, serial: &str, package: &str) -> ToolResult<Value> { // 启动应用
    let output = adb::shell(adb, serial, &["monkey", "-p", package, "-c", "android.intent.category.LAUNCHER", "1"], Duration::from_secs(30))?; // monkey 启动
    if output.contains("No activities found") || output.contains("monkey aborted") { return Err(ToolError::new("APP_NOT_FOUND", format!("应用不存在或不可启动：{package}"))); } // 启动失败
    let current = foreground(adb, serial).unwrap_or_else(|_| json!({})); // 前台确认
    Ok(json!({"package": package, "launched": true, "foreground": current})) // 返回
}
pub fn terminate(adb: &Path, serial: &str, package: &str) -> ToolResult<Value> { // 结束应用
    adb::shell(adb, serial, &["am", "force-stop", package], Duration::from_secs(20))?; // 强制停止
    Ok(json!({"package": package, "terminated": true})) // 返回
}
pub fn install(adb: &Path, serial: &str, path: &str) -> ToolResult<Value> { // 安装 APK
    if !Path::new(path).is_file() { return Err(ToolError::new("INVALID_ARGUMENT", format!("APK 不存在：{path}"))); } // 校验文件
    let output = adb::run(adb, Some(serial), &["install", "-r", "-t", path], Duration::from_secs(180))?; // 执行安装
    if !output.ok || output.text().contains("Failure") { return Err(ToolError::new("INVALID_ARGUMENT", output.error_text())); } // 失败
    Ok(json!({"path": path, "installed": true, "output": output.text().trim()})) // 返回
}
pub fn uninstall(adb: &Path, serial: &str, package: &str) -> ToolResult<Value> { // 卸载应用
    let output = adb::run(adb, Some(serial), &["uninstall", package], Duration::from_secs(60))?; // 执行卸载
    if !output.ok || output.text().contains("Failure") { return Err(ToolError::new("APP_NOT_FOUND", output.error_text())); } // 失败
    Ok(json!({"package": package, "uninstalled": true})) // 返回
}