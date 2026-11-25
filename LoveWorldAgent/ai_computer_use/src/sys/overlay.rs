use std::sync::{mpsc, OnceLock}; // 命令通道与全局句柄
use std::time::{Duration, Instant}; // 自动隐藏计时
use crate::{log_info, log_warn}; // 日志宏
use windows::core::PCWSTR; // 宽字符串
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, POINT, RECT, SIZE, WPARAM}; // 基础类型
use windows::Win32::Graphics::Dwm::DwmFlush; // 等待合成完成
use windows::Win32::Graphics::Gdi::{CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteDC, DeleteObject, DrawTextW, EnumDisplayMonitors, GetDC, GetMonitorInfoW, ReleaseDC, SelectObject, SetBkMode, SetTextColor, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, DIB_RGB_COLORS, AC_SRC_ALPHA, AC_SRC_OVER, ANTIALIASED_QUALITY, BI_RGB, DEFAULT_CHARSET, DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, FONT_CLIP_PRECISION, FONT_OUTPUT_PRECISION, HDC, HFONT, HGDIOBJ, HMONITOR, MONITORINFO, TRANSPARENT}; // 覆盖层绘制
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI}; // 每屏 DPI
use windows::Win32::UI::WindowsAndMessaging::{CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, PostMessageW, PostQuitMessage, RegisterClassW, SetTimer, ShowWindow, TranslateMessage, UpdateLayeredWindow, MSG, SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA, WM_APP, WM_TIMER, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP}; // 窗口与消息
pub const OVERLAY_CLASS_NAME: &str = "LoveWorldAiOverlay"; // 覆盖层窗口类名（list_windows 需跳过）
const CLASS_CTL: PCWSTR = windows::core::w!("LoveWorldAiOverlayCtl"); // 控制窗口类名
const CLASS_OVERLAY: PCWSTR = windows::core::w!("LoveWorldAiOverlay"); // 覆盖层窗口类名
const WAKE_MSG: u32 = WM_APP + 1; // 唤醒消息（命令入队后投递）
const TIMER_ID: usize = 1; // 定时器 id
const LINGER_MS: u64 = 1500; // 操作结束后保留时长
const GLOW_PX: f32 = 26.0; // 光晕宽度（96 DPI 基准）
const SOLID_PX: f32 = 2.0; // 实线宽度（96 DPI 基准）
const BADGE_W: f32 = 200.0; // 徽标宽（96 DPI 基准）
const BADGE_H: f32 = 32.0; // 徽标高（96 DPI 基准）
const BADGE_TOP: f32 = 24.0; // 徽标距顶部（96 DPI 基准）
const COLOR_ACCENT: (u8, u8, u8) = (0x3B, 0x82, 0xF6); // 主题蓝
const COLOR_BADGE: (u8, u8, u8) = (0x11, 0x18, 0x27); // 徽标深色底
enum Cmd { Show, Touch, Hide, Suspend(mpsc::Sender<()>), Resume, Shutdown } // 覆盖层命令
static SENDER: OnceLock<mpsc::Sender<Cmd>> = OnceLock::new(); // 全局命令发送端
static CTL: OnceLock<isize> = OnceLock::new(); // 控制窗口句柄
struct Surface { hwnd: HWND, memdc: HDC, bitmap: HGDIOBJ, old: HGDIOBJ, width: i32, height: i32, rect: RECT, scale: f32 } // 单屏绘制面
struct State { surfaces: Vec<Surface>, visible: bool, suspended: bool, quit: bool, deadline: Instant, phase: u32 } // 覆盖层状态
impl Drop for State { fn drop(&mut self) { self.destroy(); } } // 线程退出时释放资源
pub fn start() { // 启动覆盖层线程（幂等）
    SENDER.get_or_init(|| { // 只启动一次
        let (tx, rx) = mpsc::channel::<Cmd>(); // 命令通道
        let _ = std::thread::Builder::new().name("overlay".to_string()).spawn(move || run_thread(rx)); // 独立消息线程
        log_info!("overlay", "覆盖层线程已启动"); // 启动记录
        tx // 返回发送端
    });
}
pub fn show() { if let Some(tx) = SENDER.get() { let _ = tx.send(Cmd::Show); wake(); } } // 显示并刷新 deadline
pub fn touch() { if let Some(tx) = SENDER.get() { let _ = tx.send(Cmd::Touch); wake(); } } // 仅刷新 deadline
pub fn hide() { if let Some(tx) = SENDER.get() { let _ = tx.send(Cmd::Hide); wake(); } } // 立即隐藏
pub fn shutdown() { if let Some(tx) = SENDER.get() { let _ = tx.send(Cmd::Shutdown); wake(); } } // 退出线程
fn wake() { if let Some(handle) = CTL.get() { let _ = unsafe { PostMessageW(Some(HWND(*handle as *mut core::ffi::c_void)), WAKE_MSG, WPARAM(0), LPARAM(0)) }; } } // 唤醒消息循环
pub struct SuspendGuard { active: bool } // 截图避让守卫
impl Drop for SuspendGuard { fn drop(&mut self) { if self.active { if let Some(tx) = SENDER.get() { let _ = tx.send(Cmd::Resume); } wake(); } } } // 任何路径都恢复
pub fn suspend() -> SuspendGuard { // 同步隐藏覆盖层（供截图调用）
    let Some(tx) = SENDER.get() else { return SuspendGuard { active: false }; }; // 未启动直接跳过
    if CTL.get().is_none() { return SuspendGuard { active: false }; } // 线程尚未就绪
    let (ack_tx, ack_rx) = mpsc::channel(); // 隐藏完成确认
    if tx.send(Cmd::Suspend(ack_tx)).is_err() { return SuspendGuard { active: false }; } // 发送失败
    wake(); // 唤醒线程
    let _ = ack_rx.recv_timeout(Duration::from_millis(500)); // 最多等待 500ms
    SuspendGuard { active: true } // 返回守卫
}
fn run_thread(rx: mpsc::Receiver<Cmd>) { // 覆盖层消息线程
    unsafe {
        let class = WNDCLASSW { lpfnWndProc: Some(overlay_wndproc), lpszClassName: CLASS_CTL, ..Default::default() }; // 注册控制窗口类
        let overlay_class = WNDCLASSW { lpfnWndProc: Some(overlay_wndproc), lpszClassName: CLASS_OVERLAY, ..Default::default() }; // 注册覆盖层窗口类
        if RegisterClassW(&class) == 0 || RegisterClassW(&overlay_class) == 0 { log_warn!("overlay", "注册窗口类失败"); return; } // 注册失败退出
        let Ok(ctl) = CreateWindowExW(Default::default(), CLASS_CTL, windows::core::w!(""), Default::default(), 0, 0, 0, 0, Some(windows::Win32::UI::WindowsAndMessaging::HWND_MESSAGE), None, None, None) else { log_warn!("overlay", "创建控制窗口失败"); return; }; // 创建消息专用窗口
        let _ = CTL.set(ctl.0 as isize); // 记录句柄供唤醒
        log_info!("overlay", "控制窗口已就绪 hwnd={}", ctl.0 as isize); // 就绪记录
        let mut state = State { surfaces: Vec::new(), visible: false, suspended: false, quit: false, deadline: Instant::now(), phase: 0 }; // 初始化状态
        SetTimer(Some(ctl), TIMER_ID, 80, None); // 80ms 心跳（脉冲 + deadline）
        while let Ok(cmd) = rx.try_recv() { handle(cmd, &mut state); if state.quit { break; } } // 处理启动前排队的命令
        let mut msg = MSG::default(); // 消息缓冲
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 { // 消息循环
            if msg.hwnd == ctl && msg.message == WAKE_MSG { // 处理命令
                while let Ok(cmd) = rx.try_recv() { handle(cmd, &mut state); if state.quit { break; } } // 逐条执行
                if state.quit { break; } // 退出
                continue; // 已处理
            }
            if msg.hwnd == ctl && msg.message == WM_TIMER { state.tick(); continue; } // 心跳
            let _ = TranslateMessage(&msg); // 转换按键消息
            DispatchMessageW(&msg); // 分发
        }
        state.destroy(); // 释放绘制面
        let _ = DestroyWindow(ctl); // 销毁控制窗口
    }
}
fn handle(cmd: Cmd, state: &mut State) { // 执行命令
    match cmd {
        Cmd::Show => state.show(), // 显示
        Cmd::Touch => state.touch(), // 续期
        Cmd::Hide => state.hide(), // 隐藏
        Cmd::Suspend(ack) => { state.suspended = true; state.hide_windows(); unsafe { let _ = DwmFlush(); } let _ = ack.send(()); } // 隐藏并等待合成
        Cmd::Resume => { state.suspended = false; if state.visible { state.deadline = Instant::now() + Duration::from_millis(LINGER_MS); state.show_windows(); } } // 恢复
        Cmd::Shutdown => { state.quit = true; unsafe { PostQuitMessage(0); } } // 退出
    }
}
impl State {
    fn show(&mut self) { // 显示覆盖层
        self.ensure_surfaces(); // 按显示器重建
        if self.surfaces.is_empty() { log_warn!("overlay", "没有可用显示器，覆盖层未显示"); return; } // 无显示器
        self.visible = true; // 标记可见
        self.suspended = false; // 解除挂起
        self.deadline = Instant::now() + Duration::from_millis(LINGER_MS); // 刷新 deadline
        self.show_windows(); // 显示窗口
        for surface in &self.surfaces { present(surface, 230); } // 刷新首帧透明度
        log_info!("overlay", "显示覆盖层：{} 个显示器", self.surfaces.len()); // 显示记录
    }
    fn touch(&mut self) { if self.visible { self.deadline = Instant::now() + Duration::from_millis(LINGER_MS); } } // 续期
    fn hide(&mut self) { self.visible = false; self.hide_windows(); } // 隐藏
    fn show_windows(&self) { for surface in &self.surfaces { unsafe { let _ = ShowWindow(surface.hwnd, SW_SHOWNOACTIVATE); } } } // 显示全部窗口（不抢焦点）
    fn hide_windows(&self) { for surface in &self.surfaces { unsafe { let _ = ShowWindow(surface.hwnd, SW_HIDE); } } } // 隐藏全部窗口
    fn tick(&mut self) { // 心跳
        if self.visible && !self.suspended && Instant::now() > self.deadline { self.hide(); return; } // 超时隐藏
        if self.visible && !self.suspended { // 脉冲
            self.phase = (self.phase + 1) % 34; // 三角波相位
            let wave = if self.phase > 17 { 34 - self.phase } else { self.phase }; // 0..17
            let alpha = (170 + wave * 5) as u8; // 170..255
            for surface in &self.surfaces { present(surface, alpha); } // 更新透明度
        }
    }
    fn ensure_surfaces(&mut self) { // 按显示器重建绘制面
        let mut monitors: Vec<HMONITOR> = Vec::new(); // 显示器句柄
        unsafe { let _ = EnumDisplayMonitors(None, None, Some(collect_monitor), LPARAM(&mut monitors as *mut Vec<HMONITOR> as isize)); } // 枚举
        let mut rects: Vec<(RECT, f32)> = Vec::new(); // 目标矩形与缩放
        for monitor in monitors { // 逐显示器
            let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, rcMonitor: RECT::default(), rcWork: RECT::default(), dwFlags: 0 }; // 显示器信息
            if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() { continue; } // 读取失败跳过
            let mut dpi_x = 96u32; let mut dpi_y = 96u32; // DPI 输出
            let _ = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }; // 读取有效 DPI
            let scale = (dpi_x as f32 / 96.0).clamp(1.0, 3.0); // 缩放比
            rects.push((info.rcMonitor, scale)); // 记录
        }
        let unchanged = self.surfaces.len() == rects.len() && self.surfaces.iter().zip(rects.iter()).all(|(surface, (rect, scale))| surface.rect == *rect && (surface.scale - *scale).abs() < 0.01); // 是否无需重建
        if unchanged { return; } // 直接复用
        self.destroy(); // 释放旧绘制面
        for (rect, scale) in rects { if let Some(surface) = build_surface(rect, scale) { self.surfaces.push(surface); } } // 逐屏创建
        log_info!("overlay", "覆盖层绘制面已重建：{} 个显示器", self.surfaces.len()); // 记录数量
    }
    fn destroy(&mut self) { // 释放全部绘制面
        for surface in self.surfaces.drain(..) { // 逐个释放
            unsafe {
                SelectObject(surface.memdc, surface.old); // 还原位图
                let _ = DeleteObject(surface.bitmap); // 释放 DIB
                let _ = DeleteDC(surface.memdc); // 释放内存 DC
                let _ = DestroyWindow(surface.hwnd); // 销毁窗口
            }
        }
    }
}
unsafe extern "system" fn collect_monitor(monitor: HMONITOR, _hdc: HDC, _rect: *mut RECT, data: LPARAM) -> windows::core::BOOL { // 显示器枚举回调
    let list = unsafe { &mut *(data.0 as *mut Vec<HMONITOR>) }; // 还原容器
    list.push(monitor); // 收集句柄
    windows::core::BOOL(1) // 继续枚举
}
unsafe extern "system" fn overlay_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> windows::Win32::Foundation::LRESULT { // 覆盖层窗口过程
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) } // 交给默认处理（命令与定时器在消息循环中处理）
}
fn build_surface(rect: RECT, scale: f32) -> Option<Surface> { // 创建单屏覆盖层
    let width = (rect.right - rect.left).max(1); let height = (rect.bottom - rect.top).max(1); // 显示器尺寸（物理像素）
    let ex = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE; // 穿透 + 置顶 + 不抢焦点
    let hwnd = match unsafe { CreateWindowExW(ex, CLASS_OVERLAY, windows::core::w!(""), WS_POPUP, rect.left, rect.top, width, height, None, None, None, None) } { Ok(hwnd) => hwnd, Err(err) => { log_warn!("overlay", "创建覆盖层窗口失败: {err}"); return None; } }; // 创建无边框窗口
    let memdc = unsafe { CreateCompatibleDC(None) }; // 内存 DC
    if memdc.is_invalid() { unsafe { let _ = DestroyWindow(hwnd); } log_warn!("overlay", "创建内存 DC 失败"); return None; } // DC 失败回收
    let mut info = BITMAPINFO::default(); // 位图信息
    info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32; // 结构大小
    info.bmiHeader.biWidth = width; // 宽
    info.bmiHeader.biHeight = -height; // 自上而下
    info.bmiHeader.biPlanes = 1; // 单平面
    info.bmiHeader.biBitCount = 32; // 32 位
    info.bmiHeader.biCompression = BI_RGB.0; // 不压缩
    let mut bits: *mut core::ffi::c_void = std::ptr::null_mut(); // 像素指针
    let bitmap = match unsafe { CreateDIBSection(Some(memdc), &info, DIB_RGB_COLORS, &mut bits, None, 0) } { Ok(bitmap) => bitmap, Err(err) => { log_warn!("overlay", "创建 DIB 失败: {err}"); unsafe { let _ = DeleteDC(memdc); let _ = DestroyWindow(hwnd); } return None; } }; // 创建 DIB
    let old = unsafe { SelectObject(memdc, bitmap.into()) }; // 选入 DIB
    draw_surface(memdc, bits as *mut u8, width, height, scale); // 绘制边框与徽标
    let surface = Surface { hwnd, memdc, bitmap: bitmap.into(), old, width, height, rect, scale }; // 组装
    present(&surface, 255); // 首次呈现
    Some(surface) // 返回绘制面
}
static PRESENT_FAILED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false); // 只记录一次呈现失败
fn present(surface: &Surface, alpha: u8) { // 提交分层窗口位图
    unsafe {
        let screen = GetDC(None); // 屏幕 DC
        let dst = POINT { x: surface.rect.left, y: surface.rect.top }; // 目标位置
        let size = SIZE { cx: surface.width, cy: surface.height }; // 目标尺寸
        let src = POINT { x: 0, y: 0 }; // 源位置
        let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: alpha, AlphaFormat: AC_SRC_ALPHA as u8 }; // 混合参数
        let result = UpdateLayeredWindow(surface.hwnd, Some(screen), Some(&dst), Some(&size), Some(surface.memdc), Some(&src), COLORREF(0), Some(&blend), ULW_ALPHA); // 逐像素 alpha
        if let Err(err) = result { if !PRESENT_FAILED.swap(true, std::sync::atomic::Ordering::Relaxed) { log_warn!("overlay", "UpdateLayeredWindow 失败: {err}"); } } // 限流记录
        ReleaseDC(None, screen); // 释放屏幕 DC
    }
}
fn draw_surface(memdc: HDC, bits: *mut u8, width: i32, height: i32, scale: f32) { // 绘制光晕与徽标
    if bits.is_null() { return; } // 空指针保护
    let solid = (SOLID_PX * scale).round().max(1.0) as i32; // 实线宽度
    let glow = (GLOW_PX * scale).round().max(solid as f32 + 1.0) as i32; // 光晕宽度
    unsafe {
        for y in 0..height { // 逐行
            for x in 0..width { // 逐列
                let edge = x.min(y).min(width - 1 - x).min(height - 1 - y); // 到最近边缘的距离
                let alpha = if edge < solid { 235 } else if edge < glow { (((glow - edge) as f32 / glow as f32) * 110.0) as u8 } else { 0 }; // 实线 + 渐隐
                if alpha > 0 { put(bits, width, x, y, COLOR_ACCENT, alpha); } // 写入光晕像素
            }
        }
    }
    let badge_w = (BADGE_W * scale).round() as i32; // 徽标宽
    let badge_h = (BADGE_H * scale).round() as i32; // 徽标高
    let badge_x = (width - badge_w) / 2; // 水平居中
    let badge_y = (BADGE_TOP * scale).round() as i32; // 顶部偏移
    let radius = (badge_h / 2).max(2); // 圆角半径
    fill_round(bits, width, height, badge_x, badge_y, badge_w, badge_h, radius, COLOR_ACCENT, 200); // 亮边
    fill_round(bits, width, height, badge_x + 1, badge_y + 1, badge_w - 2, badge_h - 2, (radius - 1).max(1), COLOR_BADGE, 235); // 深色底
    unsafe {
        let font: HFONT = CreateFontW(-((14.0 * scale).round() as i32), 0, 0, 0, 600, 0, 0, 0, DEFAULT_CHARSET, FONT_OUTPUT_PRECISION(0), FONT_CLIP_PRECISION(0), ANTIALIASED_QUALITY, 0, windows::core::w!("Microsoft YaHei")); // 徽标字体
        if font.is_invalid() { return; } // 字体失败跳过
        let previous = SelectObject(memdc, font.into()); // 选入字体
        SetBkMode(memdc, TRANSPARENT); // 透明背景
        SetTextColor(memdc, COLORREF(0x00FF_FFFF)); // 白色文字
        let mut text: Vec<u16> = "AI 正在操作中".encode_utf16().collect(); // UTF-16 文本
        let mut rect = RECT { left: badge_x, top: badge_y, right: badge_x + badge_w, bottom: badge_y + badge_h }; // 文字矩形
        DrawTextW(memdc, &mut text, &mut rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX); // 居中绘制（GDI 不改 alpha）
        SelectObject(memdc, previous); // 还原字体
        let _ = DeleteObject(font.into()); // 释放字体
    }
}
fn fill_round(bits: *mut u8, width: i32, height: i32, x0: i32, y0: i32, w: i32, h: i32, radius: i32, rgb: (u8, u8, u8), alpha: u8) { // 填充圆角矩形
    if w <= 0 || h <= 0 { return; } // 空矩形
    let x1 = x0 + w; let y1 = y0 + h; // 右下角
    let left_cx = x0 + radius; let right_cx = x1 - radius - 1; // 左右圆心
    let top_cy = y0 + radius; let bottom_cy = y1 - radius - 1; // 上下圆心
    for y in y0.max(0)..y1.min(height) { // 逐行裁剪
        for x in x0.max(0)..x1.min(width) { // 逐列裁剪
            let cx = if x < left_cx { left_cx } else if x > right_cx { right_cx } else { x }; // 水平圆心
            let cy = if y < top_cy { top_cy } else if y > bottom_cy { bottom_cy } else { y }; // 垂直圆心
            let dx = x - cx; let dy = y - cy; // 圆心偏移
            if dx * dx + dy * dy <= radius * radius { unsafe { put(bits, width, x, y, rgb, alpha); } } // 圆角内写入
        }
    }
}
unsafe fn put(bits: *mut u8, width: i32, x: i32, y: i32, rgb: (u8, u8, u8), alpha: u8) { // 写一个 BGRA 像素
    let index = ((y * width + x) * 4) as usize; // 像素偏移
    let pixel = bits.add(index); // 像素指针
    *pixel = rgb.2; // B
    *pixel.add(1) = rgb.1; // G
    *pixel.add(2) = rgb.0; // R
    *pixel.add(3) = alpha; // A
}
