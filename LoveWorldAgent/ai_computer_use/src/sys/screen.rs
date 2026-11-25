use anyhow::{bail, Result}; // 屏幕与窗口图像采集（支持内存 PNG 与可选自动截图落盘）
use image::codecs::png::PngEncoder; // PNG 编码
use image::{ExtendedColorType, ImageEncoder}; // 编码 trait
use std::path::{Path, PathBuf}; // 路径类型
use std::sync::atomic::{AtomicU64, Ordering}; // 截图文件名序号
use windows::Win32::Foundation::RECT; // 矩形结构
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS}; // DWM 可见边界
use windows::Win32::Graphics::Gdi::{BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BI_RGB, DIB_RGB_COLORS, SRCCOPY}; // GDI 抓取
use windows::Win32::UI::HiDpi::GetDpiForSystem; // 系统 DPI
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, GetWindowRect, IsIconic, SM_CXSCREEN, SM_CXVIRTUALSCREEN, SM_CYSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN}; // 屏幕与窗口尺寸
#[link(name = "user32")]
unsafe extern "system" { // 声明 PrintWindow 以支持采集被遮挡的后台窗口内容
    fn PrintWindow(hwnd: isize, hdc_blt: isize, n_flags: u32) -> i32; // PW_RENDERFULLCONTENT=2
}
const PW_RENDERFULLCONTENT: u32 = 2; // 强制合成完整渲染内容（适配 DWM/DirectComposition/Chromium）
pub fn virtual_screen() -> (i32, i32, i32, i32) { unsafe { (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN), GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN)) } } // 虚拟桌面左上角与宽高（物理像素）
pub fn primary_screen_metrics() -> (i32, i32, f64) { // 主显示器物理像素宽高与缩放倍率
    let w = unsafe { GetSystemMetrics(SM_CXSCREEN) }.max(1); // 主屏宽
    let h = unsafe { GetSystemMetrics(SM_CYSCREEN) }.max(1); // 主屏高
    let dpi = unsafe { GetDpiForSystem() }; // 系统 DPI
    let scale = if dpi == 0 { 1.0 } else { ((dpi as f64 / 96.0) * 100.0).round() / 100.0 }; // 缩放倍率
    (w, h, scale) // 返回物理尺寸与倍率
}
pub fn window_bounds_outer(hwnd: i64) -> Option<(i32, i32, i32, i32)> { // 读取 GetWindowRect 外框 (x,y,w,h)
    let mut rect = RECT::default(); // 矩形输出
    if unsafe { GetWindowRect(crate::uia::desktop::hwnd_of(hwnd), &mut rect) }.is_err() { return None; } // 无效句柄
    Some((rect.left, rect.top, (rect.right - rect.left).max(0), (rect.bottom - rect.top).max(0))) // 返回外框
}
pub fn window_bounds_visible(hwnd: i64) -> Option<(i32, i32, i32, i32)> { // 读取 DWM 可见边框（去掉阴影扩展区）
    let mut rect = RECT::default(); // 矩形输出
    let ok = unsafe { DwmGetWindowAttribute(crate::uia::desktop::hwnd_of(hwnd), DWMWA_EXTENDED_FRAME_BOUNDS, &mut rect as *mut RECT as *mut _, std::mem::size_of::<RECT>() as u32) }.is_ok(); // 查询 DWM 边界
    if ok { Some((rect.left, rect.top, (rect.right - rect.left).max(0), (rect.bottom - rect.top).max(0))) } else { window_bounds_outer(hwnd) } // 失败回退 GetWindowRect
}
pub fn monitor_meta(hwnd: i64) -> Option<(u32, u32, f64, (i32, i32, i32, i32))> { // 窗口所在显示器 DPI、缩放与工作区
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST}; // 显示器 API
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI}; // 每屏 DPI
    let monitor = unsafe { MonitorFromWindow(crate::uia::desktop::hwnd_of(hwnd), MONITOR_DEFAULTTONEAREST) }; // 最近显示器
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, rcMonitor: Default::default(), rcWork: Default::default(), dwFlags: 0 }; // 显示器信息
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() { return None; } // 读取失败
    let mut dpi_x = 96u32; let mut dpi_y = 96u32; // DPI 输出
    let _ = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }; // 有效 DPI
    let scale = ((dpi_x as f64 / 96.0) * 100.0).round() / 100.0; // 保留两位缩放
    Some((dpi_x, dpi_y, scale, (info.rcWork.left, info.rcWork.top, info.rcWork.right - info.rcWork.left, info.rcWork.bottom - info.rcWork.top))) // 返回
}
fn encode_bgra_png(buffer: &mut [u8], width: i32, height: i32) -> Result<Vec<u8>> { // 原地将 BGRA 转为不透明 RGBA 并编码内存 PNG
    for pixel in buffer.chunks_exact_mut(4) { pixel.swap(0, 2); pixel[3] = 255; } // BGRA→RGBA 且强制不透明（GDI/PrintWindow Alpha 常为 0）
    let mut png: Vec<u8> = Vec::new(); // 内存缓冲
    PngEncoder::new(&mut png).write_image(buffer, width as u32, height as u32, ExtendedColorType::Rgba8)?; // 编码 PNG
    Ok(png) // 返回内存 PNG 字节
}
fn read_dib_bgra(memory: windows::Win32::Graphics::Gdi::HDC, bitmap: windows::Win32::Graphics::Gdi::HBITMAP, width: i32, height: i32) -> Result<Vec<u8>> { // 从 HBITMAP 读出 BGRA 像素
    let mut buffer = vec![0u8; (width as usize) * (height as usize) * 4]; // 像素缓冲
    let mut info = BITMAPINFO::default(); // 位图信息头
    info.bmiHeader.biSize = std::mem::size_of::<windows::Win32::Graphics::Gdi::BITMAPINFOHEADER>() as u32; // 头尺寸
    info.bmiHeader.biWidth = width; // 宽
    info.bmiHeader.biHeight = -height; // 负值表示自顶向下
    info.bmiHeader.biPlanes = 1; // 平面数
    info.bmiHeader.biBitCount = 32; // 32bpp
    info.bmiHeader.biCompression = BI_RGB.0; // 无压缩
    let lines = unsafe { GetDIBits(memory, bitmap, 0, height as u32, Some(buffer.as_mut_ptr() as *mut core::ffi::c_void), &mut info, DIB_RGB_COLORS) }; // 提取扫描行
    if lines == 0 { bail!("读取位图像素失败"); } // 失败返回错误
    Ok(buffer) // 返回 BGRA 字节
}
pub fn capture_region_png(x: i32, y: i32, width: i32, height: i32) -> Result<(Vec<u8>, i32, i32, u32, u32)> { // 截取屏幕区域到内存 PNG，返回 (png, origin_x, origin_y, w, h)
    let _guard = crate::sys::overlay::suspend(); // 截图期间临时隐藏覆盖层
    let (vx, vy, vw, vh) = virtual_screen(); // 虚拟桌面边界
    let width = if width <= 0 { vw } else { width }; // 缺省全宽
    let height = if height <= 0 { vh } else { height }; // 缺省全高
    let left = x.max(vx); let top = y.max(vy); // 起点钳位
    let width = width.min(vx + vw - left).max(1); let height = height.min(vy + vh - top).max(1); // 尺寸钳位
    let mut buffer = unsafe { // GDI 抓取区域
        let screen = GetDC(None); // 屏幕 DC
        if screen.is_invalid() { bail!("获取屏幕 DC 失败"); } // 失败退出
        let memory = CreateCompatibleDC(Some(screen)); // 内存 DC
        let bitmap = CreateCompatibleBitmap(screen, width, height); // 兼容位图
        let previous = SelectObject(memory, bitmap.into()); // 选入位图
        let copied = BitBlt(memory, 0, 0, width, height, Some(screen), left, top, SRCCOPY).is_ok(); // 屏幕位块传输
        let res = if copied { read_dib_bgra(memory, bitmap, width, height) } else { bail!("BitBlt 截取屏幕失败") }; // 读出像素
        SelectObject(memory, previous); // 还原位图
        let _ = DeleteObject(bitmap.into()); // 销毁位图
        let _ = DeleteDC(memory); // 销毁内存 DC
        ReleaseDC(None, screen); // 释放屏幕 DC
        res? // 返回像素缓冲
    };
    let png = encode_bgra_png(&mut buffer, width, height)?; // 编码为内存 PNG
    Ok((png, left, top, width as u32, height as u32)) // 返回内存图像及原点/尺寸
}
pub fn capture_window_png(hwnd: i64) -> Result<(Vec<u8>, i32, i32, u32, u32)> { // 抓取指定窗口到内存 PNG（PrintWindow 优先以支持被遮挡窗口，空白时回退屏幕裁剪）
    let target = crate::uia::desktop::hwnd_of(hwnd); // 转 HWND
    if unsafe { IsIconic(target) }.as_bool() { bail!("窗口 {hwnd} 当前处于最小化状态，无法识别画面，请先调用 computer_bring_to_front 恢复窗口"); } // 最小化无画面
    let (origin_x, origin_y, width, height) = window_bounds_outer(hwnd).ok_or_else(|| anyhow::anyhow!("无法读取窗口 {hwnd} 矩形"))?; // 读取窗口外框
    if width <= 0 || height <= 0 { bail!("窗口 {hwnd} 尺寸无效 ({width}x{height})"); } // 零尺寸窗口跳过
    let _guard = crate::sys::overlay::suspend(); // 截图期间隐藏覆盖层
    let mut buffer = unsafe { // 先尝试 PrintWindow
        let win_dc = GetDC(Some(target)); // 窗口 DC
        let screen_dc = if win_dc.is_invalid() { GetDC(None) } else { win_dc }; // 回退屏幕 DC
        if screen_dc.is_invalid() { bail!("获取窗口 DC 失败"); } // 失败退出
        let memory = CreateCompatibleDC(Some(screen_dc)); // 内存 DC
        let bitmap = CreateCompatibleBitmap(screen_dc, width, height); // 兼容位图
        let previous = SelectObject(memory, bitmap.into()); // 选入位图
        let pw_ok = PrintWindow(target.0 as isize, memory.0 as isize, PW_RENDERFULLCONTENT) != 0; // 优先 PrintWindow
        let mut dib = if pw_ok { read_dib_bgra(memory, bitmap, width, height).unwrap_or_default() } else { Vec::new() }; // 读取 PrintWindow 像素
        let non_blank = dib.chunks_exact(4).step_by(17).any(|px| px[0] != 0 || px[1] != 0 || px[2] != 0); // 抽样检查是否全黑空白
        if !pw_ok || !non_blank { // 若 PrintWindow 失败或返回空白则回退桌面 BitBlt
            let desktop = GetDC(None); // 桌面 DC
            if !desktop.is_invalid() { // 桌面 DC 有效
                if BitBlt(memory, 0, 0, width, height, Some(desktop), origin_x, origin_y, SRCCOPY).is_ok() { // 按外框抓取屏幕区域
                    if let Ok(fb) = read_dib_bgra(memory, bitmap, width, height) { dib = fb; } // 覆盖为空白时的桌面抓图
                }
                ReleaseDC(None, desktop); // 释放桌面 DC
            }
        }
        SelectObject(memory, previous); // 还原位图
        let _ = DeleteObject(bitmap.into()); // 释放位图
        let _ = DeleteDC(memory); // 释放内存 DC
        ReleaseDC(if win_dc.is_invalid() { None } else { Some(target) }, screen_dc); // 释放源 DC
        if dib.is_empty() { bail!("窗口图像采集为空"); } // 若仍为空则报错
        dib // 返回 BGRA 缓冲
    };
    let png = encode_bgra_png(&mut buffer, width, height)?; // 编码为内存 PNG
    Ok((png, origin_x, origin_y, width as u32, height as u32)) // 返回内存 PNG 与窗口坐标
}
static SCREENSHOT_SEQ: AtomicU64 = AtomicU64::new(1); // 自动截图递增序号
pub fn next_screenshot_seq() -> u64 { // 分配单调递增序号，防止同毫秒碰撞
    SCREENSHOT_SEQ.fetch_add(1, Ordering::Relaxed)
}
pub fn next_screenshot_filename(prefix: &str, seq: u64) -> String { // 生成带时间戳与序号的 PNG 文件名
    format!("{prefix}_{}_{seq}.png", chrono::Local::now().format("%Y%m%d_%H%M%S_%3f"))
}
pub fn capture_desktop_png() -> Result<(Vec<u8>, i32, i32, u32, u32)> { // 抓取全虚拟桌面（支持多屏负坐标）
    let (vx, vy, vw, vh) = virtual_screen();
    capture_region_png(vx, vy, vw, vh)
}
pub fn save_png_to_dir(dir: &Path, filename: &str, bytes: &[u8]) -> Result<PathBuf> { // 确保目录存在并写入 PNG 字节
    std::fs::create_dir_all(dir)?;
    let target = dir.join(filename);
    std::fs::write(&target, bytes)?;
    Ok(target)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn next_screenshot_filename_has_clean_timestamp_and_unique_seq() {
        let a = next_screenshot_filename("cu", 1);
        let b = next_screenshot_filename("cu", 2);
        assert!(a.starts_with("cu_"));
        assert!(a.ends_with("_1.png"));
        assert!(b.ends_with("_2.png"));
        assert_ne!(a, b);
    }
    #[test]
    fn save_png_to_dir_creates_missing_dir_and_writes_bytes() {
        let dir = std::env::temp_dir().join(format!("lwa_screen_test_{}", std::process::id())).join("nested");
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
        let saved = save_png_to_dir(&dir, "shot_1.png", b"\x89PNG\r\n\x1a\n").unwrap();
        assert_eq!(saved, dir.join("shot_1.png"));
        assert_eq!(std::fs::read(&saved).unwrap(), b"\x89PNG\r\n\x1a\n");
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}