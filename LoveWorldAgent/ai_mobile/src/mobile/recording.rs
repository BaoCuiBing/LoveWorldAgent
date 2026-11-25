use serde_json::{json, Value}; // JSON 结果
use std::path::{Path, PathBuf}; // 路径
use std::process::{Child, Command, Stdio}; // 录屏子进程
use std::sync::{Mutex, OnceLock}; // 录屏状态
use std::time::Duration; // 超时
use crate::config; // 静态目录
use crate::mobile::adb::{self, ToolError, ToolResult}; // adb 与错误
const REMOTE_PATH: &str = "/sdcard/lwa_screenrecord.mp4"; // 设备端临时文件
static RECORDING: OnceLock<Mutex<Option<Child>>> = OnceLock::new(); // 录制进程
fn slot() -> &'static Mutex<Option<Child>> { RECORDING.get_or_init(|| Mutex::new(None)) } // 取槽位
fn output_dir() -> PathBuf { config::detect_paths().map(|paths| paths.static_dir.join("data").join("mobile_screen")).unwrap_or_else(|| config::exe_dir().join("data").join("mobile_screen")) } // 输出目录
pub fn start(adb: &Path, serial: &str, limit_seconds: u64) -> ToolResult<Value> { // 开始录屏
    let mut guard = slot().lock().map_err(|_| ToolError::new("INVALID_ARGUMENT", "录屏状态锁异常"))?; // 取锁
    if let Some(child) = guard.as_mut() { if child.try_wait().ok().flatten().is_none() { return Err(ToolError::new("INVALID_ARGUMENT", "已在录屏中，请先停止")); } } // 已在录制
    let _ = adb::shell(adb, serial, &["rm", "-f", REMOTE_PATH], Duration::from_secs(10)); // 清理旧文件
    let limit = limit_seconds.clamp(5, 180).to_string(); // 时长限制
    let child = Command::new(adb) // 构造后台进程
        .arg("-s").arg(serial) // 指定设备
        .args(["shell", "screenrecord", "--time-limit", &limit, REMOTE_PATH]) // 录制命令
        .stdout(Stdio::null()).stderr(Stdio::null()) // 静默
        .spawn().map_err(|err| ToolError::new("ADB_NOT_FOUND", format!("启动录屏失败：{err}")))?; // 拉起
    *guard = Some(child); // 保存
    Ok(json!({"recording": true, "remote": REMOTE_PATH, "limitSeconds": limit_seconds})) // 返回
}
pub fn stop(adb: &Path, serial: &str) -> ToolResult<Value> { // 停止录屏并拉取
    let mut guard = slot().lock().map_err(|_| ToolError::new("INVALID_ARGUMENT", "录屏状态锁异常"))?; // 取锁
    let Some(mut child) = guard.take() else { return Err(ToolError::new("INVALID_ARGUMENT", "当前没有进行中的录屏")); }; // 无录制
    let _ = child.kill(); // 结束录屏
    let _ = child.wait(); // 回收
    let dir = output_dir(); // 输出目录
    std::fs::create_dir_all(&dir).map_err(|err| ToolError::new("INVALID_ARGUMENT", format!("创建目录失败：{err}")))?; // 建目录
    let name = format!("mobile_recording_{}.mp4", chrono::Utc::now().timestamp()); // 文件名
    let local = dir.join(&name); // 本地路径
    let local_text = local.to_string_lossy().to_string(); // 本地路径文本
    let output = adb::run(adb, Some(serial), &["pull", REMOTE_PATH, &local_text], Duration::from_secs(120))?; // 拉取
    if !output.ok { return Err(ToolError::new("ADB_TIMEOUT", output.error_text())); } // 拉取失败
    let _ = adb::shell(adb, serial, &["rm", "-f", REMOTE_PATH], Duration::from_secs(10)); // 清理设备文件
    Ok(json!({"recording": false, "path": local.to_string_lossy(), "bytes": std::fs::metadata(&local).map(|item| item.len()).unwrap_or(0)})) // 返回
}
pub fn active() -> bool { slot().lock().map(|guard| guard.is_some()).unwrap_or(false) } // 是否录制中