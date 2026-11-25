use anyhow::{bail, Result}; // Office 窗口自动截图采集（仅截取可见非最小化窗口，无桌面回退）
use image::codecs::png::PngEncoder; // PNG 编码
use image::{ExtendedColorType, ImageEncoder}; // 编码 trait
use std::path::{Path, PathBuf}; // 路径类型
use std::sync::atomic::{AtomicU64, Ordering}; // 截图文件名序号
use windows::Win32::Foundation::{HWND, RECT}; // Win32 句柄与矩形
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HDC, HBITMAP,
    SRCCOPY,
}; // GDI 采集
use windows::Win32::UI::WindowsAndMessaging::{
    GetAncestor, GetWindowRect, IsIconic, IsWindow, IsWindowVisible, GA_ROOT,
}; // 窗口校验与根窗口查询
#[link(name = "user32")]
unsafe extern "system" {
    fn PrintWindow(hwnd: isize, hdc_blt: isize, n_flags: u32) -> i32; // PW_RENDERFULLCONTENT=2
}
const PW_RENDERFULLCONTENT: u32 = 2; // 强制合成完整渲染内容（适配 DWM/Office GPU 渲染）
static SCREENSHOT_SEQ: AtomicU64 = AtomicU64::new(1); // 自动截图递增序号

pub fn hwnd_of(hwnd: isize) -> HWND {
    HWND(hwnd as *mut core::ffi::c_void)
}

pub fn root_window(hwnd: isize) -> isize { // 归一化为顶层根窗口（避免 Excel MDI 子窗句柄）
    if hwnd <= 0 {
        return 0;
    }
    let root = unsafe { GetAncestor(hwnd_of(hwnd), GA_ROOT) };
    if root.is_invalid() || root.0.is_null() {
        hwnd
    } else {
        root.0 as isize
    }
}

pub fn window_bounds_outer(hwnd: isize) -> Option<(i32, i32, i32, i32)> { // 读取窗口外框 (x,y,w,h)
    if hwnd <= 0 {
        return None;
    }
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd_of(hwnd), &mut rect) }.is_err() {
        return None;
    }
    Some((
        rect.left,
        rect.top,
        (rect.right - rect.left).max(0),
        (rect.bottom - rect.top).max(0),
    ))
}

pub fn is_capturable_window(hwnd: isize) -> bool { // 校验句柄有效、可见、未最小化且宽高均大于 0
    if hwnd <= 0 {
        return false;
    }
    let target = hwnd_of(hwnd);
    let valid_visible = unsafe {
        IsWindow(Some(target)).as_bool()
            && IsWindowVisible(target).as_bool()
            && !IsIconic(target).as_bool()
    };
    if !valid_visible {
        return false;
    }
    window_bounds_outer(hwnd)
        .map(|(_, _, width, height)| width > 0 && height > 0)
        .unwrap_or(false)
}

fn encode_bgra_png(buffer: &mut [u8], width: i32, height: i32) -> Result<Vec<u8>> { // 原地将 BGRA 转为不透明 RGBA 并编码内存 PNG
    for pixel in buffer.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        pixel[3] = 255;
    }
    let mut png: Vec<u8> = Vec::new();
    PngEncoder::new(&mut png).write_image(
        buffer,
        width as u32,
        height as u32,
        ExtendedColorType::Rgba8,
    )?;
    Ok(png)
}

fn read_dib_bgra(memory: HDC, bitmap: HBITMAP, width: i32, height: i32) -> Result<Vec<u8>> { // 从 HBITMAP 读出 BGRA 像素
    let mut buffer = vec![0u8; (width as usize) * (height as usize) * 4];
    let mut info = BITMAPINFO::default();
    info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = width;
    info.bmiHeader.biHeight = -height;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB.0;
    let lines = unsafe {
        GetDIBits(
            memory,
            bitmap,
            0,
            height as u32,
            Some(buffer.as_mut_ptr() as *mut core::ffi::c_void),
            &mut info,
            DIB_RGB_COLORS,
        )
    };
    if lines == 0 {
        bail!("读取位图像素失败");
    }
    Ok(buffer)
}

pub fn capture_window_png(hwnd: isize) -> Result<Vec<u8>> { // 抓取指定可见 Office 窗口到内存 PNG（PrintWindow 优先，空白时按窗口矩形 BitBlt 回退）
    if !is_capturable_window(hwnd) {
        bail!("窗口 {hwnd} 无效、不可见或已最小化，跳过截图");
    }
    let (origin_x, origin_y, width, height) = window_bounds_outer(hwnd)
        .ok_or_else(|| anyhow::anyhow!("无法读取窗口 {hwnd} 矩形"))?;
    if width <= 0 || height <= 0 {
        bail!("窗口 {hwnd} 尺寸无效 ({width}x{height})");
    }
    let target = hwnd_of(hwnd);
    let mut buffer = unsafe {
        let win_dc = GetDC(Some(target));
        let screen_dc = if win_dc.is_invalid() { GetDC(None) } else { win_dc };
        if screen_dc.is_invalid() {
            bail!("获取窗口 DC 失败");
        }
        let memory = CreateCompatibleDC(Some(screen_dc));
        if memory.is_invalid() {
            ReleaseDC(if win_dc.is_invalid() { None } else { Some(target) }, screen_dc);
            bail!("创建兼容 DC 失败");
        }
        let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
        if bitmap.is_invalid() {
            let _ = DeleteDC(memory);
            ReleaseDC(if win_dc.is_invalid() { None } else { Some(target) }, screen_dc);
            bail!("创建兼容位图失败");
        }
        let previous = SelectObject(memory, bitmap.into());
        let pw_ok = PrintWindow(target.0 as isize, memory.0 as isize, PW_RENDERFULLCONTENT) != 0;
        let mut dib = if pw_ok {
            read_dib_bgra(memory, bitmap, width, height).unwrap_or_default()
        } else {
            Vec::new()
        };
        let non_blank = dib
            .chunks_exact(4)
            .step_by(17)
            .any(|px| px[0] != 0 || px[1] != 0 || px[2] != 0);
        if (!pw_ok || !non_blank) && is_capturable_window(hwnd) {
            let desktop = GetDC(None);
            if !desktop.is_invalid() {
                if BitBlt(
                    memory,
                    0,
                    0,
                    width,
                    height,
                    Some(desktop),
                    origin_x,
                    origin_y,
                    SRCCOPY,
                )
                .is_ok()
                {
                    if let Ok(fb) = read_dib_bgra(memory, bitmap, width, height) {
                        dib = fb;
                    }
                }
                ReleaseDC(None, desktop);
            }
        }
        SelectObject(memory, previous);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(memory);
        ReleaseDC(if win_dc.is_invalid() { None } else { Some(target) }, screen_dc);
        if dib.is_empty() {
            bail!("窗口图像采集为空");
        }
        dib
    };
    encode_bgra_png(&mut buffer, width, height)
}

pub fn next_screenshot_seq() -> u64 { // 分配单调递增序号
    SCREENSHOT_SEQ.fetch_add(1, Ordering::Relaxed)
}

pub fn next_screenshot_filename(prefix: &str, seq: u64) -> String { // 生成带时间戳与序号的 PNG 文件名
    format!(
        "{prefix}_{}_{seq}.png",
        chrono::Local::now().format("%Y%m%d_%H%M%S_%3f")
    )
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
    fn is_capturable_window_rejects_zero_and_invalid_hwnd() {
        assert!(!is_capturable_window(0));
        assert!(!is_capturable_window(-1));
        assert!(!is_capturable_window(0x7FFF_1234));
    }
    #[test]
    fn next_screenshot_filename_includes_family_prefix_and_unique_seq() {
        let a = next_screenshot_filename("word", 1);
        let b = next_screenshot_filename("ppt", 2);
        assert!(a.starts_with("word_"));
        assert!(a.ends_with("_1.png"));
        assert!(b.starts_with("ppt_"));
        assert!(b.ends_with("_2.png"));
        assert_ne!(a, b);
    }
    #[test]
    fn save_png_to_dir_creates_missing_dir_and_writes_bytes() {
        let dir = std::env::temp_dir()
            .join(format!("lwa_office_screen_test_{}", std::process::id()))
            .join("nested");
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
        let saved = save_png_to_dir(&dir, "word_1.png", b"\x89PNG\r\n\x1a\n").unwrap();
        assert_eq!(saved, dir.join("word_1.png"));
        assert_eq!(std::fs::read(&saved).unwrap(), b"\x89PNG\r\n\x1a\n");
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
    #[test]
    fn capture_window_png_skips_invalid_hwnd_without_panic() {
        assert!(capture_window_png(0).is_err());
        assert!(capture_window_png(-42).is_err());
        assert!(capture_window_png(0x7FFF_1234).is_err());
    }
}
