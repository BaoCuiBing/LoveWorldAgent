use std::io::Read; // 读取子进程输出
use std::path::{Path, PathBuf}; // 可执行文件路径
use std::process::{Command, Stdio}; // 命令与标准流
use std::thread; // 输出读取线程
use std::time::{Duration, Instant}; // 超时控制
use crate::config; // 静态目录定位
#[derive(Debug, Clone)] // 统一错误（对外映射为错误码）
pub struct ToolError { // 工具错误
    pub code: &'static str, // 机器可读错误码
    pub message: String, // 中文说明
}
impl ToolError { // 错误构造与格式化
    pub fn new(code: &'static str, message: impl Into<String>) -> Self { Self { code, message: message.into() } } // 构造
    pub fn text(&self) -> String { format!("{}: {}", self.code, self.message) } // 管道错误文本
}
pub type ToolResult<T> = Result<T, ToolError>; // 结果别名
#[derive(Debug, Clone)] // 命令执行结果
pub struct Output { // 子进程输出
    pub ok: bool, // 退出码是否为 0
    pub code: i32, // 退出码
    pub stdout: Vec<u8>, // 原始标准输出（截图二进制）
    pub stderr: String, // 标准错误文本
}
impl Output { // 便捷读取
    pub fn text(&self) -> String { String::from_utf8_lossy(&self.stdout).to_string() } // 标准输出转文本
    pub fn error_text(&self) -> String { let text = self.stderr.trim(); if text.is_empty() { self.text().trim().to_string() } else { text.to_string() } } // 失败文本
}
#[derive(Debug, Clone)] // 设备条目
pub struct Device { // adb devices 行
    pub serial: String, // 序列号
    pub model: String, // 型号（device 后 model: 字段）
}
fn exe_in_dir(dir: &Path) -> Option<PathBuf> { let path = dir.join("adb.exe"); if path.is_file() { Some(path) } else { None } } // 目录内 adb.exe
pub fn resolve_adb(configured: Option<&str>) -> ToolResult<PathBuf> { // adb 解析：配置 → ANDROID_HOME/SDK_ROOT → PATH → 随包
    if let Some(raw) = configured.map(str::trim).filter(|text| !text.is_empty()) { // 显式配置优先
        let path = PathBuf::from(raw); // 配置路径
        if path.is_file() { return Ok(path); } // 命中
        if path.is_dir() { if let Some(found) = exe_in_dir(&path) { return Ok(found); } } // 允许传目录
        return Err(ToolError::new("ADB_NOT_FOUND", format!("配置的 adb 不存在：{raw}"))); // 配置失效
    }
    for key in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] { // 环境变量
        if let Ok(root) = std::env::var(key) { // 读取环境变量
            if let Some(found) = exe_in_dir(&Path::new(root.trim()).join("platform-tools")) { return Ok(found); } // platform-tools/adb.exe
        }
    }
    let probe = Command::new("adb").arg("version").stdout(Stdio::null()).stderr(Stdio::null()).status(); // PATH 探测
    if probe.map(|status| status.success()).unwrap_or(false) { return Ok(PathBuf::from("adb")); } // 直接使用 PATH 中的 adb
    if let Some(paths) = config::detect_paths() { // 随包副本
        if let Some(found) = exe_in_dir(&paths.static_dir.join("mobile").join("platform-tools")) { return Ok(found); } // static/mobile/platform-tools/adb.exe
    }
    Err(ToolError::new("ADB_NOT_FOUND", "未找到 adb.exe：请安装 platform-tools 或在设置中填写 adb 路径")) // 全部失败
}
pub fn run(adb: &Path, serial: Option<&str>, args: &[&str], timeout: Duration) -> ToolResult<Output> { // 同步执行 adb 并带超时
    let mut command = Command::new(adb); // 构造命令
    if let Some(value) = serial { command.arg("-s").arg(value); } // 指定设备
    command.args(args).stdout(Stdio::piped()).stderr(Stdio::piped()); // 参数与管道
    let mut child = command.spawn().map_err(|err| ToolError::new("ADB_NOT_FOUND", format!("启动 adb 失败：{err}")))?; // 拉起进程
    let stdout = child.stdout.take().map(|mut pipe| thread::spawn(move || { let mut buffer = Vec::new(); let _ = pipe.read_to_end(&mut buffer); buffer })); // 读 stdout
    let stderr = child.stderr.take().map(|mut pipe| thread::spawn(move || { let mut buffer = Vec::new(); let _ = pipe.read_to_end(&mut buffer); buffer })); // 读 stderr
    let deadline = Instant::now() + timeout; // 截止时刻
    let status = loop { // 轮询等待
        match child.try_wait() { // 非阻塞检查
            Ok(Some(status)) => break status, // 已退出
            Ok(None) => { // 仍在运行
                if Instant::now() >= deadline { // 超时
                    let _ = child.kill(); // 结束进程
                    let _ = child.wait(); // 回收
                    return Err(ToolError::new("ADB_TIMEOUT", format!("adb 执行超时（{}s）", timeout.as_secs()))); // 超时错误
                }
                thread::sleep(Duration::from_millis(20)); // 避免忙等
            }
            Err(err) => return Err(ToolError::new("ADB_TIMEOUT", format!("等待 adb 退出失败：{err}"))), // 等待失败
        }
    };
    let out = stdout.map(|handle| handle.join().unwrap_or_default()).unwrap_or_default(); // 标准输出
    let err = stderr.map(|handle| handle.join().unwrap_or_default()).unwrap_or_default(); // 标准错误
    Ok(Output { ok: status.success(), code: status.code().unwrap_or(-1), stdout: out, stderr: String::from_utf8_lossy(&err).to_string() }) // 组装结果
}
pub fn devices(adb: &Path) -> ToolResult<Vec<Device>> { // 列出在线设备
    let output = run(adb, None, &["devices", "-l"], Duration::from_secs(15))?; // 执行命令
    if !output.ok { return Err(ToolError::new("ADB_NOT_FOUND", output.error_text())); } // 命令失败
    let mut list = Vec::new(); // 结果
    for line in output.text().lines().skip(1) { // 跳过首行标题
        let parts: Vec<&str> = line.split_whitespace().collect(); // 拆字段
        if parts.len() < 2 || parts[1] != "device" { continue; } // 仅取 state=device
        let model = parts.iter().find_map(|item| item.strip_prefix("model:")).unwrap_or("").to_string(); // 型号
        list.push(Device { serial: parts[0].to_string(), model }); // 收集
    }
    Ok(list) // 返回
}
pub fn select(adb: &Path, preferred: Option<&str>) -> ToolResult<String> { // 选择目标设备
    let list = devices(adb)?; // 在线设备
    if let Some(raw) = preferred.map(str::trim).filter(|text| !text.is_empty()) { // 显式设备
        if list.iter().any(|item| item.serial == raw) { return Ok(raw.to_string()); } // 命中
        return Err(ToolError::new("DEVICE_NOT_FOUND", format!("设备不在线：{raw}"))); // 不在线
    }
    match list.len() { // 未指定设备
        0 => Err(ToolError::new("DEVICE_NOT_FOUND", "未发现已授权且在线设备（请开启 USB 调试）")), // 无设备
        1 => Ok(list[0].serial.clone()), // 唯一设备
        _ => Err(ToolError::new("DEVICE_REQUIRED", format!("检测到 {} 台设备，请显式指定 device", list.len()))), // 多设备
    }
}
pub fn shell(adb: &Path, serial: &str, args: &[&str], timeout: Duration) -> ToolResult<String> { // 执行 adb shell 并返回文本
    let mut full = vec!["shell"]; // shell 子命令
    full.extend_from_slice(args); // 追加参数
    let output = run(adb, Some(serial), &full, timeout)?; // 执行
    if !output.ok { return Err(ToolError::new("ADB_TIMEOUT", output.error_text())); } // 失败
    Ok(output.text()) // 返回文本
}