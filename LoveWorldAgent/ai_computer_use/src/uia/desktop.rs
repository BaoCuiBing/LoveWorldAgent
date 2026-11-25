use anyhow::Result; // 错误类型
use serde::Serialize; // 序列化派生
use windows::core::{BOOL, PWSTR}; // 布尔与宽字符串指针
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM}; // 基础类型
use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION}; // 进程信息
use windows::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP}; // 前台权限唤醒
use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetAncestor, GetClassNameW, GetForegroundWindow, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, SetForegroundWindow, ShowWindow, GA_ROOT, SW_RESTORE}; // 窗口枚举与激活
#[derive(Debug, Clone, Serialize)] // 窗口信息
pub struct WindowInfo { // 顶层窗口描述
    pub hwnd: i64, // 窗口句柄（转为 i64 便于 JSON）
    pub title: String, // 窗口标题
    pub class_name: String, // 窗口类名
    pub process_id: u32, // 进程 ID
    pub process_name: String, // 进程名
    pub visible: bool, // 是否可见
    pub x: i32, // 左坐标
    pub y: i32, // 上坐标
    pub width: i32, // 宽度
    pub height: i32, // 高度
}
#[derive(Debug, Clone, Default)] // 窗口过滤条件
pub struct WindowFilter { // 对应 uia_get_desktop 参数
    pub title: Option<String>, // 标题模糊匹配
    pub class_name: Option<String>, // 类名精确匹配
    pub process_name: Option<String>, // 进程名忽略大小写
    pub only_visible: bool, // 仅可见窗口
}
pub fn list_windows(filter: &WindowFilter) -> Vec<WindowInfo> { // 枚举并过滤顶层窗口
    let mut windows: Vec<WindowInfo> = Vec::new(); // 结果容器
    let pointer = &mut windows as *mut Vec<WindowInfo> as isize; // 传给回调的指针
    unsafe { let _ = EnumWindows(Some(enum_window), LPARAM(pointer)); } // 枚举顶层窗口
    windows.into_iter().filter(|window| matches_filter(window, filter)).collect() // 应用过滤条件
}
fn matches_filter(window: &WindowInfo, filter: &WindowFilter) -> bool { // 判断窗口是否命中过滤
    if window.class_name == crate::sys::overlay::OVERLAY_CLASS_NAME { return false; } // 跳过自身覆盖层
    if filter.only_visible && !window.visible { return false; } // 仅可见
    if let Some(title) = filter.title.as_ref() { // 标题过滤
        if !window.title.to_lowercase().contains(&title.to_lowercase()) { return false; } // 模糊匹配
    }
    if let Some(class_name) = filter.class_name.as_ref() { // 类名过滤
        if !window.class_name.eq_ignore_ascii_case(class_name) { return false; } // 精确匹配
    }
    if let Some(process_name) = filter.process_name.as_ref() { // 进程名过滤
        if !window.process_name.eq_ignore_ascii_case(process_name) { return false; } // 忽略大小写
    }
    true // 全部命中
}
unsafe extern "system" fn enum_window(hwnd: HWND, lparam: LPARAM) -> BOOL { // EnumWindows 回调
    let windows = unsafe { &mut *(lparam.0 as *mut Vec<WindowInfo>) }; // 还原结果容器
    if let Some(info) = window_info(hwnd) { windows.push(info); } // 采集窗口信息
    BOOL(1) // 继续枚举
}
fn window_info(hwnd: HWND) -> Option<WindowInfo> { // 读取单个窗口信息
    unsafe { // 调用 Win32 API
        let mut title_buffer = [0u16; 512]; // 标题缓冲
        let title_len = GetWindowTextW(hwnd, &mut title_buffer); // 读取标题
        let title = String::from_utf16_lossy(&title_buffer[..title_len.max(0) as usize]); // 转为字符串
        let mut class_buffer = [0u16; 256]; // 类名缓冲
        let class_len = GetClassNameW(hwnd, &mut class_buffer); // 读取类名
        let class_name = String::from_utf16_lossy(&class_buffer[..class_len.max(0) as usize]); // 转为字符串
        let mut process_id = 0u32; // 进程 ID
        GetWindowThreadProcessId(hwnd, Some(&mut process_id)); // 读取进程 ID
        let mut rect_value = windows::Win32::Foundation::RECT::default(); // 窗口矩形
        let _ = GetWindowRect(hwnd, &mut rect_value); // 读取窗口矩形
        Some(WindowInfo { // 构造窗口信息
            hwnd: hwnd.0 as isize as i64, // 句柄转 i64
            title, // 标题
            class_name, // 类名
            process_id, // 进程 ID
            process_name: process_name(process_id).unwrap_or_default(), // 进程名
            visible: IsWindowVisible(hwnd).as_bool(), // 可见状态
            x: rect_value.left, // 左坐标
            y: rect_value.top, // 上坐标
            width: rect_value.right - rect_value.left, // 宽度
            height: rect_value.bottom - rect_value.top, // 高度
        })
    }
}
fn process_name(process_id: u32) -> Option<String> { // 读取进程可执行文件名
    if process_id == 0 { return None; } // 无进程 ID
    unsafe { // 调用 Win32 API
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id).ok()?; // 打开进程
        let mut buffer = [0u16; 1024]; // 路径缓冲
        let mut size = buffer.len() as u32; // 缓冲长度
        let result = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buffer.as_mut_ptr()), &mut size); // 查询完整路径
        let _ = CloseHandle(handle); // 关闭句柄
        result.ok()?; // 失败返回 None
        let full_path = String::from_utf16_lossy(&buffer[..size as usize]); // 转为字符串
        Some(full_path.rsplit(['\\', '/']).next().unwrap_or(&full_path).to_string()) // 只保留文件名
    }
}
pub fn process_name_of(pid: u32) -> String { process_name(pid).unwrap_or_default() } // 进程可执行文件名
pub fn capture_region(_x: i32, _y: i32, _width: i32, _height: i32) -> Result<String> { // 区域截图接口
    anyhow::bail!("not implemented: screenshot 尚未实现（P1 里程碑，计划 BitBlt + PNG 编码后返回 base64）") // 明确未实现
}
pub fn hwnd_of(value: i64) -> HWND { HWND(value as isize as *mut core::ffi::c_void) } // i64 转窗口句柄
pub fn window_title(hwnd: i64) -> String { // 读取窗口标题
    let mut buffer = [0u16; 512]; // 标题缓冲
    let len = unsafe { GetWindowTextW(hwnd_of(hwnd), &mut buffer) }; // 读取标题
    String::from_utf16_lossy(&buffer[..len.max(0) as usize]) // 转为字符串
}
pub fn window_rect(hwnd: i64) -> Option<(i32, i32, i32, i32)> { // 读取窗口矩形
    let mut rect = windows::Win32::Foundation::RECT::default(); // 矩形缓冲
    unsafe { GetWindowRect(hwnd_of(hwnd), &mut rect).ok()?; } // 读取成功才返回
    Some((rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top)) // 返回左/上/宽/高
}
pub fn foreground_window() -> i64 { unsafe { GetForegroundWindow() }.0 as isize as i64 } // 前台窗口
pub fn windows_by_pid(pid: u32) -> Vec<WindowInfo> { // 该进程的可见顶层窗口
    list_windows(&WindowFilter { only_visible: true, ..Default::default() }).into_iter().filter(|window| window.process_id == pid).collect() // 过滤进程
}
pub fn activate(hwnd: i64) -> bool { // 激活目标窗口（恢复最小化并置前）
    if hwnd == 0 { return false; } // 无效句柄
    let handle = hwnd_of(hwnd); // 窗口句柄
    unsafe { // 调用 Win32
        if IsIconic(handle).as_bool() { let _ = ShowWindow(handle, SW_RESTORE); } // 最小化时恢复
        if GetForegroundWindow() == handle { return true; } // 已在前台，避免 Alt 触发 KeyTip
        for attempt in 0..8 { // 前台锁可能短暂失败，重试
            if SetForegroundWindow(handle).as_bool() && GetForegroundWindow() == handle { return true; } // 成功
            if attempt == 0 { keybd_event(0x12, 0, KEYBD_EVENT_FLAGS(0), 0); keybd_event(0x12, 0, KEYEVENTF_KEYUP, 0); } // 首次失败时用 Alt 解除前台锁
            std::thread::sleep(std::time::Duration::from_millis(50)); // 短暂等待
        }
        GetForegroundWindow() == handle // 最后一次判定
    }
}
pub fn main_window_of(pid: u32) -> Option<WindowInfo> { // pid 的主窗口：前台优先，否则首个可见窗口
    let windows = windows_by_pid(pid); // 该进程窗口
    let foreground = foreground_window(); // 前台窗口
    windows.iter().find(|window| window.hwnd == foreground).cloned().or_else(|| windows.into_iter().next()) // 前台优先
}
pub fn pid_of_window(hwnd: i64) -> u32 { let mut pid = 0u32; unsafe { GetWindowThreadProcessId(hwnd_of(hwnd), Some(&mut pid)); } pid } // 窗口所属进程
pub fn is_minimized(hwnd: i64) -> bool { unsafe { IsIconic(hwnd_of(hwnd)).as_bool() } } // 是否最小化
pub fn root_window(hwnd: i64) -> i64 { // 取顶层祖先窗口（0 表示无效）
    if hwnd == 0 { return 0; } // 无句柄
    unsafe { GetAncestor(hwnd_of(hwnd), GA_ROOT).0 as isize as i64 } // GA_ROOT
}
pub fn window_process_name(hwnd: i64) -> String { process_name(pid_of_window(hwnd)).unwrap_or_default() } // 窗口进程名
pub fn is_frame_host_pid(pid: u32) -> bool { // 是否为 UWP ApplicationFrameHost 宿主
    windows_by_pid(pid).iter().any(|window| window.class_name.eq_ignore_ascii_case("ApplicationFrameWindow")) // 宿主窗口类名
}
pub fn is_frame_host_window(hwnd: i64) -> bool { // 窗口是否由 UWP 宿主承载
    pid_of_window(hwnd) != 0 && is_frame_host_pid(pid_of_window(hwnd)) // 按宿主 pid 判断
}
