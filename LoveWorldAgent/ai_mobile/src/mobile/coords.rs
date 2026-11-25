use crate::mobile::adb::{self, ToolError, ToolResult}; // adb 与错误
use std::time::Duration; // 超时
#[derive(Debug, Clone, Copy)] // 尺寸
pub struct Size { // 宽高
    pub width: u32, // 宽
    pub height: u32, // 高
}
pub fn png_size(bytes: &[u8]) -> Option<Size> { // 读取 PNG IHDR 宽高
    if bytes.len() < 24 { return None; } // 长度不足
    if &bytes[0..8] != b"\x89PNG\r\n\x1a\n" { return None; } // PNG 签名
    if &bytes[12..16] != b"IHDR" { return None; } // IHDR 块
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]); // 宽
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]); // 高
    Some(Size { width, height }) // 返回
}
pub fn screen_size(adb: &std::path::Path, serial: &str) -> ToolResult<Size> { // 读取设备真实分辨率
    let text = adb::shell(adb, serial, &["wm", "size"], Duration::from_secs(15))?; // wm size
    for line in text.lines() { // 优先 Physical size
        let line = line.trim(); // 去空白
        if let Some(raw) = line.strip_prefix("Physical size:") { // 物理分辨率
            if let Some(size) = parse_size(raw.trim()) { return Ok(size); } // 解析成功
        }
    }
    for line in text.lines() { // 回退 Override size
        if let Some(raw) = line.trim().strip_prefix("Override size:") { if let Some(size) = parse_size(raw.trim()) { return Ok(size); } } // 覆盖分辨率
    }
    Err(ToolError::new("INVALID_ARGUMENT", "无法解析 wm size 输出")) // 解析失败
}
fn parse_size(raw: &str) -> Option<Size> { // 解析 WxH
    let (width, height) = raw.split_once('x')?; // 拆分
    Some(Size { width: width.trim().parse().ok()?, height: height.trim().parse().ok()? }) // 转数字
}
pub fn scale_to_device(x: f64, y: f64, shot: Size, device: Size) -> (i64, i64) { // 截图坐标换算到设备坐标
    let sx = device.width as f64 / shot.width.max(1) as f64; // X 比例
    let sy = device.height as f64 / shot.height.max(1) as f64; // Y 比例
    ((x * sx).round() as i64, (y * sy).round() as i64) // 四舍五入
}
pub fn clamp(value: i64, max: u32) -> i64 { value.max(0).min(max.saturating_sub(1) as i64) } // 限制到屏幕内