use anyhow::{anyhow, bail, Result}; // 键鼠输入（覆盖前台 SendInput 与后台 PostMessage/WM_SETTEXT）
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM}; // 窗口与消息参数
use windows::Win32::Graphics::Gdi::ScreenToClient; // 屏幕坐标转客户区
use windows::Win32::UI::Controls::EM_REPLACESEL; // 编辑框插入文本
use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MOUSEINPUT, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSE_EVENT_FLAGS, VIRTUAL_KEY}; // 输入 API
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, PostMessageW, SendMessageW, SetCursorPos, WM_HSCROLL, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETTEXT, WM_VSCROLL}; // 光标定位与消息投递
pub fn cursor_position() -> (i32, i32) { // 读取当前系统鼠标坐标（屏幕物理像素）
    let mut pt = POINT { x: 0, y: 0 }; // 输出点
    unsafe { let _ = GetCursorPos(&mut pt); } // 调用 Win32 GetCursorPos
    (pt.x, pt.y) // 返回坐标
}
pub fn move_to(x: i32, y: i32) -> Result<()> { unsafe { SetCursorPos(x, y)?; } Ok(()) } // 移动鼠标到屏幕坐标
fn mouse_event(flags: MOUSE_EVENT_FLAGS, data: i32) -> Result<()> { let input = INPUT { r#type: INPUT_MOUSE, Anonymous: INPUT_0 { mi: MOUSEINPUT { dx: 0, dy: 0, mouseData: data as u32, dwFlags: flags, time: 0, dwExtraInfo: 0 } } }; let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) }; if sent == 0 { bail!("发送鼠标事件失败"); } Ok(()) } // 单条鼠标事件
pub fn click(x: i32, y: i32, button: &str, clicks: usize) -> Result<()> { move_to(x, y)?; let (down, up) = match button.to_ascii_lowercase().as_str() { "right" => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP), "middle" => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP), _ => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP) }; for _ in 0..clicks.max(1) { mouse_event(down, 0)?; mouse_event(up, 0)?; std::thread::sleep(std::time::Duration::from_millis(30)); } Ok(()) } // 坐标点击（支持按键与连击）
pub fn click_with_modifiers(x: i32, y: i32, button: &str, clicks: usize, modifiers: &[String]) -> Result<()> { // 带修饰键的前台点击
    let codes: Vec<u16> = modifiers.iter().map(|m| vk_of(m)).collect::<Result<Vec<u16>>>()?; // 解析修饰键码
    for code in &codes { key_event(*code, KEYBD_EVENT_FLAGS(0))?; } // 按下修饰键
    let res = click(x, y, button, clicks); // 执行点击
    for code in codes.iter().rev() { let _ = key_event(*code, KEYEVENTF_KEYUP); } // 逆序抬起
    res // 返回点击结果
}
pub fn scroll(x: i32, y: i32, delta: i32) -> Result<()> { move_to(x, y)?; mouse_event(MOUSEEVENTF_WHEEL, delta)?; Ok(()) } // 滚轮（delta 正上负下）
pub fn scroll_horizontal(x: i32, y: i32, delta: i32) -> Result<()> { move_to(x, y)?; mouse_event(MOUSEEVENTF_HWHEEL, delta)?; Ok(()) } // 横向滚轮（delta 正右负左）
pub fn scroll_ticks(x: i32, y: i32, direction: &str, by: &str, amount: i32) -> Result<()> { // 按方向与步长滚动滚轮
    let scale = if by.eq_ignore_ascii_case("page") { 5 } else { 1 }; // 按页滚动倍率
    let mag = (amount.clamp(1, 50) * 120 * scale).max(120); // WHEEL_DELTA 基准 120
    match direction.to_ascii_lowercase().as_str() { // 映射四向滚轮
        "up" => scroll(x, y, mag), // 向上
        "down" => scroll(x, y, -mag), // 向下
        "right" => scroll_horizontal(x, y, mag), // 向右
        "left" => scroll_horizontal(x, y, -mag), // 向左
        other => bail!("不支持的滚动方向: {other}"), // 非法方向
    }
}
pub fn background_scroll(hwnd: isize, direction: &str, by: &str, amount: i32) -> Result<()> { // 后台通过 WM_VSCROLL/WM_HSCROLL 滚动窗口
    if hwnd == 0 { bail!("窗口句柄无效"); } // 无句柄
    let handle = HWND(hwnd as *mut core::ffi::c_void); // 转句柄
    let is_page = by.eq_ignore_ascii_case("page"); // 是否翻页
    let (msg, code): (u32, usize) = match direction.to_ascii_lowercase().as_str() { // 映射滚动消息与命令码
        "up" => (WM_VSCROLL, if is_page { 2 } else { 0 }), // SB_PAGEUP=2 / SB_LINEUP=0
        "down" => (WM_VSCROLL, if is_page { 3 } else { 1 }), // SB_PAGEDOWN=3 / SB_LINEDOWN=1
        "left" => (WM_HSCROLL, if is_page { 2 } else { 0 }), // SB_PAGELEFT=2 / SB_LINELEFT=0
        "right" => (WM_HSCROLL, if is_page { 3 } else { 1 }), // SB_PAGERIGHT=3 / SB_LINERIGHT=1
        other => bail!("不支持的滚动方向: {other}"), // 非法方向
    };
    for _ in 0..amount.clamp(1, 50) { unsafe { PostMessageW(Some(handle), msg, WPARAM(code), LPARAM(0))?; } } // 投递指定次数
    Ok(()) // 完成
}
fn key_event(vk: u16, flags: KEYBD_EVENT_FLAGS) -> Result<()> { let input = INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(vk), wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } } }; let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) }; if sent == 0 { bail!("发送键盘事件失败"); } Ok(()) } // 单条键盘事件
pub fn type_text(text: &str) -> Result<()> { type_text_delay(text, 0) } // Unicode 逐字输入（中文与表情均可）
pub fn type_text_delay(text: &str, delay_ms: u64) -> Result<()> { // 按可配置间隔输入
    for unit in text.encode_utf16() { // 逐 UTF-16 单元
        let input = INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(0), wScan: unit, dwFlags: KEYEVENTF_UNICODE, time: 0, dwExtraInfo: 0 } } }; // 按下
        let up = INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(0), wScan: unit, dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 } } }; // 抬起
        let items = [input, up]; // 事件对
        let sent = unsafe { SendInput(&items, std::mem::size_of::<INPUT>() as i32) }; // 发送
        if sent == 0 { bail!("发送 Unicode 输入失败"); } // 失败
        if delay_ms > 0 { std::thread::sleep(std::time::Duration::from_millis(delay_ms)); } // 慢速输入间隔
    }
    Ok(()) // 完成
}
pub fn type_text_multiline(text: &str, delay_ms: u64) -> Result<()> { // 支持换行的文本输入
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n"); // 统一换行
    let parts: Vec<&str> = normalized.split('\n').collect(); // 按行拆分
    for (index, part) in parts.iter().enumerate() { // 逐行输入
        type_text_delay(part, delay_ms)?; // 当前行
        if index + 1 < parts.len() { press_key("Enter")?; if delay_ms > 0 { std::thread::sleep(std::time::Duration::from_millis(delay_ms)); } } // 换行插回车
    }
    Ok(()) // 完成
}
pub fn replace_selection(hwnd: isize, text: &str) -> Result<()> { // 后台向原生 Edit 接收者插入文本
    if hwnd == 0 { bail!("元素没有原生 Edit 接收者"); } // 无句柄
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect(); // 宽字符串
    unsafe { SendMessageW(HWND(hwnd as *mut core::ffi::c_void), EM_REPLACESEL, Some(WPARAM(1)), Some(LPARAM(wide.as_ptr() as isize))) }; // EM_REPLACESEL
    Ok(()) // 完成
}
pub fn background_click(hwnd: isize, screen_x: i32, screen_y: i32, button: &str, clicks: usize) -> Result<()> { // 后台鼠标点击
    if hwnd == 0 { bail!("窗口句柄无效"); } // 无句柄
    let handle = HWND(hwnd as *mut core::ffi::c_void); // 窗口句柄
    let mut point = POINT { x: screen_x, y: screen_y }; // 屏幕坐标
    if unsafe { ScreenToClient(handle, &mut point) }.0 == 0 { bail!("屏幕坐标转客户区失败"); } // 转换失败
    let position = ((point.y as isize & 0xFFFF) << 16) | (point.x as isize & 0xFFFF); // MAKELPARAM
    let (down, up) = match button.to_ascii_lowercase().as_str() { "right" => (WM_RBUTTONDOWN, WM_RBUTTONUP), "middle" => (WM_MBUTTONDOWN, WM_MBUTTONUP), _ => (WM_LBUTTONDOWN, WM_LBUTTONUP) }; // 按键消息
    for _ in 0..clicks.max(1) { unsafe { PostMessageW(Some(handle), down, WPARAM(0), LPARAM(position))?; PostMessageW(Some(handle), up, WPARAM(0), LPARAM(position))?; } std::thread::sleep(std::time::Duration::from_millis(30)); } // 投递按下/抬起
    Ok(()) // 完成
}
pub fn background_key(hwnd: isize, vk: u16, down: bool) -> Result<()> { // 后台按键消息
    if hwnd == 0 { bail!("窗口句柄无效"); } // 无句柄
    let message = if down { WM_KEYDOWN } else { WM_KEYUP }; // 消息类型
    unsafe { PostMessageW(Some(HWND(hwnd as *mut core::ffi::c_void)), message, WPARAM(vk as usize), LPARAM(0))?; } // 投递
    Ok(()) // 完成
}
pub fn background_type_text(hwnd: isize, text: &str) -> Result<()> { // 后台投递 WM_CHAR（用于 SendInput 不可用或非前景编辑框）
    if hwnd == 0 { bail!("窗口句柄无效"); } // 无句柄
    let handle = HWND(hwnd as *mut core::ffi::c_void); // 转 HWND
    for unit in text.encode_utf16() { unsafe { PostMessageW(Some(handle), windows::Win32::UI::WindowsAndMessaging::WM_CHAR, WPARAM(unit as usize), LPARAM(0))?; } } // 逐 UTF-16 投递
    Ok(()) // 完成
}
pub fn vk_of(token: &str) -> Result<u16> { // 按键名转虚拟键码
    let text = token.trim(); // 去空白
    if text.is_empty() { bail!("按键名称为空"); } // 空名拒绝
    let upper = text.to_ascii_uppercase(); // 转大写
    if upper.len() == 1 { let ch = upper.as_bytes()[0]; if ch.is_ascii_alphanumeric() { return Ok(ch as u16); } } // 单字母/数字
    let code = match upper.as_str() { "ENTER" | "RETURN" => 0x0D, "TAB" => 0x09, "ESC" | "ESCAPE" => 0x1B, "SPACE" => 0x20, "BACKSPACE" => 0x08, "DELETE" | "DEL" => 0x2E, "INSERT" => 0x2D, "HOME" => 0x24, "END" => 0x23, "PAGEUP" | "PRIOR" => 0x21, "PAGEDOWN" | "NEXT" => 0x22, "UP" => 0x26, "DOWN" => 0x28, "LEFT" => 0x25, "RIGHT" => 0x27, "CTRL" | "CONTROL" | "CMD" | "COMMAND" => 0x11, "SHIFT" => 0x10, "ALT" | "MENU" => 0x12, "WIN" | "LWIN" | "WINDOWS" | "META" | "SUPER" => 0x5B, "F1" => 0x70, "F2" => 0x71, "F3" => 0x72, "F4" => 0x73, "F5" => 0x74, "F6" => 0x75, "F7" => 0x76, "F8" => 0x77, "F9" => 0x78, "F10" => 0x79, "F11" => 0x7A, "F12" => 0x7B, _ => return Err(anyhow!("未知按键: {text}")) }; // 常见命名键映射
    Ok(code) // 返回虚拟键码
}
pub fn press_key(combo: &str) -> Result<()> { // 组合键（以 + 分隔）：顺序按下再逆序抬起
    let tokens: Vec<String> = combo.split('+').map(|item| item.trim().to_string()).filter(|item| !item.is_empty()).collect(); // 拆分键值
    press_keys_slice(&tokens) // 复用切片实现
}
pub fn press_keys_slice(tokens: &[String]) -> Result<()> { // 按传入切片依次按下再逆序抬起
    if tokens.is_empty() { bail!("组合键为空"); } // 空组合键拒绝
    let codes: Vec<u16> = tokens.iter().map(|token| vk_of(token)).collect::<Result<Vec<u16>>>()?; // 映射键码
    for code in &codes { key_event(*code, KEYBD_EVENT_FLAGS(0))?; } // 依次按下
    for code in codes.iter().rev() { key_event(*code, KEYEVENTF_KEYUP)?; } // 逆序释放
    Ok(()) // 完成
}
pub fn drag(from_x: i32, from_y: i32, to_x: i32, to_y: i32, button: &str, duration_ms: u64) -> Result<()> { // 兼容默认 20 步拖拽
    drag_steps(from_x, from_y, to_x, to_y, button, duration_ms, 20, &[]) // 委托带步数实现
}
pub fn drag_steps(from_x: i32, from_y: i32, to_x: i32, to_y: i32, button: &str, duration_ms: u64, steps: usize, modifiers: &[String]) -> Result<()> { // 按步数平滑拖拽（支持修饰键与左/右/中键）
    let (down, up) = match button.to_ascii_lowercase().as_str() { "right" => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP), "middle" => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP), _ => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP) }; // 键位掩码
    let mod_codes: Vec<u16> = modifiers.iter().map(|m| vk_of(m)).collect::<Result<Vec<u16>>>()?; // 解析修饰键
    for code in &mod_codes { key_event(*code, KEYBD_EVENT_FLAGS(0))?; } // 按下修饰键
    let res = (|| -> Result<()> { // 执行拖动闭环保证释放修饰键
        move_to(from_x, from_y)?; // 移至起点
        mouse_event(down, 0)?; // 按下鼠标键
        let count = steps.clamp(1, 200) as u64; // 步数限制 1..=200
        let delay = (duration_ms / count).max(1); // 每步休眠
        for step in 1..=count { // 插值移动
            let x = from_x + ((to_x - from_x) as i64 * step as i64 / count as i64) as i32; // X 插值
            let y = from_y + ((to_y - from_y) as i64 * step as i64 / count as i64) as i32; // Y 插值
            unsafe { SetCursorPos(x, y)?; } // 移动指针
            if duration_ms > 0 { std::thread::sleep(std::time::Duration::from_millis(delay)); } // 步间停顿
        }
        mouse_event(up, 0)?; // 松开鼠标键
        Ok(()) // 完成
    })();
    for code in mod_codes.iter().rev() { let _ = key_event(*code, KEYEVENTF_KEYUP); } // 逆序释放修饰键
    res // 返回拖拽结果
}
pub fn set_text_background(hwnd: isize, text: &str) -> Result<()> { // 后台写入窗口文本
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect(); // 宽字符
    let result = unsafe { SendMessageW(HWND(hwnd as *mut core::ffi::c_void), WM_SETTEXT, Some(WPARAM(0)), Some(LPARAM(wide.as_ptr() as isize))) }; // 发送 WM_SETTEXT
    if result.0 != 1 { bail!("WM_SETTEXT 未确认"); } // 失败报错
    Ok(()) // 完成
}
pub fn select_text_keyboard(text: &str, prefix: &str, suffix: &str, start: Option<usize>, end: Option<usize>) -> Result<()> { // 键盘文本选区辅助
    press_key("Ctrl+A")?; // 全选
    if let (Some(from), Some(to)) = (start, end) { press_key("Ctrl+Home")?; for _ in 0..from { press_key("Right")?; } for _ in from..to { press_key("Shift+Right")?; } } else if !text.is_empty() { press_key("Ctrl+F")?; type_text(prefix)?; press_key("Enter")?; press_key("Esc")?; type_text(text)?; } else if !prefix.is_empty() || !suffix.is_empty() { press_key("Ctrl+Home")?; type_text(prefix)?; let count = suffix.chars().count(); for _ in 0..count { press_key("Shift+Right")?; } } // 按模式选区
    Ok(()) // 完成
}