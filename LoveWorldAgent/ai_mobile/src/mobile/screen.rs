use base64::Engine; // 截图 base64
use chrono::{FixedOffset, Utc}; // 文件时间戳
use serde::Serialize; // 尺寸序列化
use std::path::{Path, PathBuf}; // 路径
use std::sync::{Mutex, OnceLock}; // 最近截图尺寸缓存
use std::time::Duration; // 超时
use crate::config; // 静态目录
use crate::mobile::adb::{self, ToolError, ToolResult}; // adb 与错误
use crate::mobile::coords::{self, Size}; // 尺寸换算
static LAST_SHOT: OnceLock<Mutex<Option<Size>>> = OnceLock::new(); // 最近截图尺寸
fn last() -> &'static Mutex<Option<Size>> { LAST_SHOT.get_or_init(|| Mutex::new(None)) } // 缓存句柄
pub fn last_size() -> Option<Size> { last().lock().ok().and_then(|guard| *guard) } // 读取最近截图尺寸
#[derive(Serialize)] // 屏幕信息
pub struct ScreenInfo { // 设备屏幕
    pub width: u32, // 宽
    pub height: u32, // 高
    pub density: String, // 密度
}
pub fn screenshot_bytes(adb: &Path, serial: &str) -> ToolResult<Vec<u8>> { // 抓取 PNG 原始字节
    let output = adb::run(adb, Some(serial), &["exec-out", "screencap", "-p"], Duration::from_secs(40))?; // exec-out
    if !output.ok || output.stdout.is_empty() { return Err(ToolError::new("ADB_TIMEOUT", output.error_text())); } // 失败
    if let Some(size) = coords::png_size(&output.stdout) { if let Ok(mut guard) = last().lock() { *guard = Some(size); } } // 记录尺寸
    Ok(output.stdout) // 返回字节
}
pub fn capture(adb: &Path, serial: &str) -> ToolResult<serde_json::Value> { // 截图并返回 base64
    let bytes = screenshot_bytes(adb, serial)?; // 抓图
    let size = coords::png_size(&bytes).ok_or_else(|| ToolError::new("INVALID_ARGUMENT", "截图不是有效 PNG"))?; // 尺寸
    let device = coords::screen_size(adb, serial)?; // 设备分辨率
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes); // base64
    Ok(serde_json::json!({"mime": "image/png", "base64": encoded, "bytes": bytes.len(), "screenshot": {"width": size.width, "height": size.height}, "device": {"width": device.width, "height": device.height}})) // 返回
}
fn output_dir() -> PathBuf { config::detect_paths().map(|paths| paths.static_dir.join("data").join("mobile_screen")).unwrap_or_else(|| config::exe_dir().join("data").join("mobile_screen")) } // 截图目录
pub fn save(adb: &Path, serial: &str, filename: Option<&str>) -> ToolResult<serde_json::Value> { // 截图落盘
    let bytes = screenshot_bytes(adb, serial)?; // 抓图
    let size = coords::png_size(&bytes).ok_or_else(|| ToolError::new("INVALID_ARGUMENT", "截图不是有效 PNG"))?; // 尺寸
    let dir = output_dir(); // 目录
    std::fs::create_dir_all(&dir).map_err(|err| ToolError::new("INVALID_ARGUMENT", format!("创建截图目录失败：{err}")))?; // 建目录
    let stamp = Utc::now().with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap()).format("%Y%m%d_%H%M%S_%3f").to_string(); // 时间戳
    let name = filename.map(|text| text.trim().to_string()).filter(|text| !text.is_empty()).unwrap_or_else(|| format!("mobile_{stamp}.png")); // 文件名
    let path = dir.join(name); // 完整路径
    std::fs::write(&path, &bytes).map_err(|err| ToolError::new("INVALID_ARGUMENT", format!("写入截图失败：{err}")))?; // 落盘
    Ok(serde_json::json!({"path": path.to_string_lossy(), "bytes": bytes.len(), "width": size.width, "height": size.height})) // 返回
}
pub fn info(adb: &Path, serial: &str) -> ToolResult<ScreenInfo> { // 屏幕分辨率与密度
    let size = coords::screen_size(adb, serial)?; // 分辨率
    let density = adb::shell(adb, serial, &["wm", "density"], Duration::from_secs(15)).map(|text| text.trim().to_string()).unwrap_or_default(); // 密度
    Ok(ScreenInfo { width: size.width, height: size.height, density }) // 返回
}
pub fn orientation(adb: &Path, serial: &str) -> ToolResult<serde_json::Value> { // 当前方向
    let text = adb::shell(adb, serial, &["dumpsys", "input"], Duration::from_secs(20)).unwrap_or_default(); // 输入状态
    let mut rotation = 0i64; // 默认竖屏
    for line in text.lines() { // 逐行查找
        let line = line.trim(); // 去空白
        if let Some(rest) = line.strip_prefix("SurfaceOrientation:") { rotation = rest.trim().parse().unwrap_or(0); break; } // 读取
    }
    let label = if rotation == 0 { "portrait" } else { "landscape" }; // 文案
    Ok(serde_json::json!({"rotation": rotation, "orientation": label})) // 返回
}
pub fn set_orientation(adb: &Path, serial: &str, value: &str) -> ToolResult<serde_json::Value> { // 设置方向
    let rotation = match value.trim().to_lowercase().as_str() { // 映射
        "portrait" | "portrait_up" => "0", // 竖屏
        "landscape" | "landscape_left" => "1", // 横屏
        "reverse_portrait" | "portrait_down" => "2", // 反向竖屏
        "reverse_landscape" | "landscape_right" => "3", // 反向横屏
        _ => return Err(ToolError::new("INVALID_ARGUMENT", "orientation 仅支持 portrait/landscape 等")), // 参数错误
    };
    adb::shell(adb, serial, &["settings", "put", "system", "accelerometer_rotation", "0"], Duration::from_secs(15))?; // 关闭自动旋转
    adb::shell(adb, serial, &["settings", "put", "system", "user_rotation", rotation], Duration::from_secs(15))?; // 写入方向
    orientation(adb, serial) // 回读
}