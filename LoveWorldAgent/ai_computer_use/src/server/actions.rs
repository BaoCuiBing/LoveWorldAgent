use serde_json::{json, Map, Value}; // 键鼠与元素语义动作工具实现（click/double_click/right_click/drag/type_text/press_key/hotkey/scroll/set_value/move_cursor/invoke_menu）
use windows::core::BSTR; // COM 字符串
use windows::Win32::UI::Accessibility::{IUIAutomationExpandCollapsePattern, IUIAutomationInvokePattern, IUIAutomationScrollPattern, IUIAutomationSelectionItemPattern, IUIAutomationTogglePattern, IUIAutomationValuePattern, ScrollAmount_LargeDecrement, ScrollAmount_LargeIncrement, ScrollAmount_NoAmount, ScrollAmount_SmallDecrement, ScrollAmount_SmallIncrement, TreeScope_Descendants, UIA_ControlTypePropertyId, UIA_ExpandCollapsePatternId, UIA_InvokePatternId, UIA_MenuBarControlTypeId, UIA_MenuControlTypeId, UIA_MenuItemControlTypeId, UIA_ScrollPatternId, UIA_SelectionItemPatternId, UIA_TogglePatternId, UIA_ValuePatternId}; // UIA 动作 Pattern 与菜单枚举
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsIconic}; // 前台与最小化状态
use crate::sys::{overlay, screen}; // 覆盖层与窗口矩形
use crate::uia::{desktop, input, worker::UiaHandle}; // 桌面、输入与 UIA 句柄
use super::{apps, snapshots}; // 窗口校验与令牌解析
fn arg_u32(args: &Map<String, Value>, key: &str) -> Option<u32> { args.get(key).and_then(Value::as_u64).map(|v| v as u32) } // 读取可选 u32
fn arg_i64(args: &Map<String, Value>, key: &str) -> Option<i64> { args.get(key).and_then(Value::as_i64) } // 读取可选 i64
fn arg_f64(args: &Map<String, Value>, key: &str) -> Option<f64> { args.get(key).and_then(Value::as_f64) } // 读取可选 f64
fn arg_str(args: &Map<String, Value>, key: &str) -> Option<String> { args.get(key).and_then(Value::as_str).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) } // 读取可选非空字符串
fn arg_strs(args: &Map<String, Value>, key: &str) -> Vec<String> { args.get(key).and_then(Value::as_array).map(|arr| arr.iter().filter_map(Value::as_str).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default() } // 读取字符串数组
fn is_foreground(args: &Map<String, Value>) -> bool { arg_str(args, "delivery_mode").map(|s| s.eq_ignore_ascii_case("foreground")).unwrap_or(false) } // 是否显式要求前台输入
fn is_desktop_scope(args: &Map<String, Value>) -> bool { arg_str(args, "scope").map(|s| s.eq_ignore_ascii_case("desktop")).unwrap_or(false) && args.get("pid").is_none() && args.get("window_id").is_none() } // 无窗桌面作用域判定
fn local_to_screen(hwnd: i64, x: f64, y: f64) -> Result<(i32, i32), String> { // 将窗口相对坐标转为屏幕物理坐标
    let (ox, oy, w, h) = screen::window_bounds_outer(hwnd).ok_or_else(|| format!("无法读取 window_id {hwnd} 坐标矩形"))?; // 读取外框（与 capture_window_png 同原点）
    if w <= 0 || h <= 0 { return Err(format!("window_id {hwnd} 尺寸无效")); } // 空窗拒绝
    Ok((ox + x.round() as i32, oy + y.round() as i32)) // 偏移相加
}
async fn focus_for_foreground(hwnd: i64) -> Option<i64> { // 临时切向前台，返回旧前台窗口以便后续恢复
    let prev = unsafe { GetForegroundWindow() }.0 as isize as i64; // 记录原前台窗
    if prev != hwnd { let _ = desktop::activate(hwnd); tokio::time::sleep(std::time::Duration::from_millis(80)).await; } // 激活并略等焦点稳定
    if prev != 0 && prev != hwnd { Some(prev) } else { None } // 返回旧窗
}
fn restore_foreground(prev: Option<i64>) { // 若原前台不同则在安全投递后恢复
    if let Some(h) = prev { let _ = desktop::activate(h); } // 尽量恢复用户原焦点窗口
}
async fn click_element_uia(uia: &UiaHandle, element_id: i64, expand: bool) -> Result<String, String> { // 通过 UIA Invoke/ExpandCollapse/Toggle/SelectionItem 后台激活元素
    uia.call(move |worker| {
        let el = worker.get(element_id)?; // 取缓存元素
        if expand { // action == "expand" 走展开折叠模式
            let pat: IUIAutomationExpandCollapsePattern = unsafe { el.GetCurrentPatternAs(UIA_ExpandCollapsePatternId) }.map_err(|_| anyhow::anyhow!("元素不支持 ExpandCollapsePattern"))?; // 获取展开模式
            unsafe { pat.Expand() }?; // 展开下拉/菜单
            return Ok(json!({"path": "ax", "action": "expand"})); // 返回
        }
        if let Ok(pat) = unsafe { el.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId) } { unsafe { pat.Invoke() }?; return Ok(json!({"path": "ax", "action": "invoke"})); } // 优先 Invoke
        if let Ok(pat) = unsafe { el.GetCurrentPatternAs::<IUIAutomationTogglePattern>(UIA_TogglePatternId) } { unsafe { pat.Toggle() }?; return Ok(json!({"path": "ax", "action": "toggle"})); } // 其次 Toggle
        if let Ok(pat) = unsafe { el.GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(UIA_SelectionItemPatternId) } { unsafe { pat.Select() }?; return Ok(json!({"path": "ax", "action": "select"})); } // 再次 Select
        anyhow::bail!("元素不支持 Invoke/Toggle/SelectionItem 语义激活") // 回退像素点击
    }).await.map(|v| v.get("path").and_then(Value::as_str).unwrap_or("ax").to_string()).map_err(|e| e.to_string()) // 返回通道名
}
pub async fn click_common(uia: &UiaHandle, args: &Map<String, Value>, forced_button: Option<&str>, forced_count: Option<usize>) -> Value { // 统一处理 computer_click / double_click / right_click
    overlay::show(); // 点亮操作覆盖层
    let button = forced_button.map(str::to_string).or_else(|| arg_str(args, "button")).unwrap_or_else(|| "left".to_string()); // 按键类型
    let count = forced_count.unwrap_or_else(|| args.get("count").and_then(Value::as_u64).unwrap_or(1).clamp(1, 3) as usize); // 连击次数
    let modifiers = arg_strs(args, "modifier"); // 修饰键
    let fg = is_foreground(args); // 是否前台模式
    let has_xy = args.get("x").and_then(Value::as_f64).is_some() && args.get("y").and_then(Value::as_f64).is_some(); // 是否传了像素坐标
    if has_xy && args.get("pid").is_none() && args.get("window_id").is_none() && !is_desktop_scope(args) && arg_str(args, "element_token").is_none() {
        return json!({"ok": false, "error": "desktop_coordinate_scope_required: 无 pid/window_id 且传 (x,y) 时必须显式指定 scope=\"desktop\""}); // 拒绝无域裸坐标
    }
    if !modifiers.is_empty() && !fg && !is_desktop_scope(args) {
        return json!({"ok": false, "error": "background_unavailable: 后台 PostMessage 无法承载 Live 修饰键状态，带 modifier 时请改用 delivery_mode=\"foreground\""}); // 修饰键后台拒绝
    }
    if is_desktop_scope(args) { // 1) 全桌面屏幕坐标分支
        let (Some(x), Some(y)) = (arg_f64(args, "x"), arg_f64(args, "y")) else { return json!({"ok": false, "error": "desktop 作用域点击必须同时提供 x 与 y"}); }; // 校验 xy
        let (sx, sy) = (x.round() as i32, y.round() as i32); // 屏幕坐标
        let btn_in = button.clone(); // 移入线程副本
        return match tokio::task::spawn_blocking(move || input::click_with_modifiers(sx, sy, &btn_in, count, &modifiers)).await {
            Ok(Ok(())) => json!({"ok": true, "data": {"scope": "desktop", "path": "send_input", "x": sx, "y": sy, "button": button, "count": count}}),
            Ok(Err(e)) => json!({"ok": false, "error": e.to_string()}),
            Err(e) => json!({"ok": false, "error": e.to_string()}),
        };
    }
    let pid_opt = arg_u32(args, "pid"); // 可选 pid
    if let Some(token) = arg_str(args, "element_token") { // 2) element_token 寻址分支
        let resolved = match snapshots::resolve_token(&token, pid_opt) { Ok(r) => r, Err(err) => return json!({"ok": false, "error": err}) }; // 解析令牌
        let expand = arg_str(args, "action").map(|a| a.eq_ignore_ascii_case("expand")).unwrap_or(false); // 是否展开动作
        if !fg && button == "left" && count == 1 && modifiers.is_empty() { // 单左键默认走 UIA 后台语义调用
            if let Ok(path_name) = click_element_uia(uia, resolved.element.element_id, expand).await {
                return json!({"ok": true, "data": {"path": path_name, "element_token": token, "pid": resolved.pid, "window_id": resolved.hwnd, "button": button, "count": count}}); // 后台无感完成
            }
        }
        let (cx, cy) = resolved.element.center; // 回退到缓存元素中心像素点
        let hwnd = resolved.hwnd; // 窗口句柄
        if !fg && modifiers.is_empty() && !unsafe { IsIconic(desktop::hwnd_of(hwnd)) }.as_bool() { // 尝试 PostMessage 后台点击
            if input::background_click(hwnd as isize, cx, cy, &button, count).is_ok() {
                return json!({"ok": true, "data": {"path": "post_message", "element_token": token, "pid": resolved.pid, "window_id": hwnd, "x": cx, "y": cy, "button": button, "count": count}});
            }
        }
        let prev = focus_for_foreground(hwnd).await; // 前台激活点击并尽量还原焦点
        let click_res = tokio::task::spawn_blocking(move || input::click_with_modifiers(cx, cy, &button, count, &modifiers)).await; // 物理点击
        restore_foreground(prev); // 还原旧前台
        return match click_res {
            Ok(Ok(())) => json!({"ok": true, "data": {"path": "foreground_fallback", "element_token": token, "pid": resolved.pid, "window_id": hwnd, "screen_x": cx, "screen_y": cy}}),
            Ok(Err(e)) => json!({"ok": false, "error": e.to_string()}),
            Err(e) => json!({"ok": false, "error": e.to_string()}),
        };
    }
    let (Some(x), Some(y)) = (arg_f64(args, "x"), arg_f64(args, "y")) else { return json!({"ok": false, "error": "点击需要提供 element_token 或成对的 (x, y) 坐标"}); }; // 3) 窗口局部像素分支
    let (pid, hwnd) = match apps::resolve_pid_hwnd(pid_opt, arg_i64(args, "window_id")) { Ok(v) => v, Err(err) => return json!({"ok": false, "error": err}) }; // 解析目标窗
    let (sx, sy) = match local_to_screen(hwnd, x, y) { Ok(p) => p, Err(err) => return json!({"ok": false, "error": err}) }; // 转屏幕坐标
    if !fg && modifiers.is_empty() && button == "left" && count == 1 { // 后台像素点击先试 UIA 命中点 Invoke
        let hit_res = uia.call(move |worker| {
            let el = worker.element_from_point(sx, sy)?; // 坐标命中元素
            if let Ok(pat) = unsafe { el.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId) } { unsafe { pat.Invoke() }?; return Ok(json!({"invoked": true})); } // 尝试 Invoke
            Ok(json!({"invoked": false})) // 非 Invoke 元素
        }).await;
        if hit_res.as_ref().ok().and_then(|v| v.get("invoked")).and_then(Value::as_bool) == Some(true) {
            return json!({"ok": true, "data": {"path": "ax_point", "pid": pid, "window_id": hwnd, "screen_x": sx, "screen_y": sy, "button": button, "count": count}}); // UWP/XAML 免切焦点命中
        }
    }
    if !fg && modifiers.is_empty() && !unsafe { IsIconic(desktop::hwnd_of(hwnd)) }.as_bool() { // 尝试 PostMessage
        if input::background_click(hwnd as isize, sx, sy, &button, count).is_ok() {
            return json!({"ok": true, "data": {"path": "post_message", "pid": pid, "window_id": hwnd, "screen_x": sx, "screen_y": sy, "button": button, "count": count}});
        }
    }
    let prev = focus_for_foreground(hwnd).await; // 前台物理点击
    let btn_in = button.clone(); // 移入线程副本
    let click_res = tokio::task::spawn_blocking(move || input::click_with_modifiers(sx, sy, &btn_in, count, &modifiers)).await; // 执行点击
    restore_foreground(prev); // 还原旧前台
    match click_res {
        Ok(Ok(())) => json!({"ok": true, "data": {"path": "foreground", "pid": pid, "window_id": hwnd, "screen_x": sx, "screen_y": sy, "button": button, "count": count}}),
        Ok(Err(e)) => json!({"ok": false, "error": e.to_string()}),
        Err(e) => json!({"ok": false, "error": e.to_string()}),
    }
}
pub async fn drag(args: &Map<String, Value>) -> Value { // computer_drag：支持 desktop 绝对坐标与 window 局部坐标拖拽
    overlay::show(); // 显示覆盖层
    let (Some(fx), Some(fy), Some(tx), Some(ty)) = (arg_f64(args, "from_x"), arg_f64(args, "from_y"), arg_f64(args, "to_x"), arg_f64(args, "to_y")) else { return json!({"ok": false, "error": "drag 缺少 from_x/from_y/to_x/to_y 参数"}); }; // 校验起止点
    let duration_ms = args.get("duration_ms").and_then(Value::as_u64).unwrap_or(500).min(10_000); // 默认 500ms
    let steps = args.get("steps").and_then(Value::as_u64).unwrap_or(20).clamp(1, 200) as usize; // 默认 20 步
    let button = arg_str(args, "button").unwrap_or_else(|| "left".to_string()); // 按键
    let modifiers = arg_strs(args, "modifier"); // 修饰键
    if !modifiers.is_empty() && !is_foreground(args) && !is_desktop_scope(args) {
        return json!({"ok": false, "error": "background_unavailable: 后台拖拽不支持携带 modifier 修饰键，请改用 delivery_mode=\"foreground\""}); // 修饰键后台拒绝
    }
    if is_desktop_scope(args) { // 桌面绝对坐标拖拽
        let (sx1, sy1, sx2, sy2) = (fx.round() as i32, fy.round() as i32, tx.round() as i32, ty.round() as i32); // 坐标取整
        return match tokio::task::spawn_blocking(move || input::drag_steps(sx1, sy1, sx2, sy2, &button, duration_ms, steps, &modifiers)).await {
            Ok(Ok(())) => json!({"ok": true, "data": {"scope": "desktop", "from": [sx1, sy1], "to": [sx2, sy2], "steps": steps}}),
            Ok(Err(e)) => json!({"ok": false, "error": e.to_string()}),
            Err(e) => json!({"ok": false, "error": e.to_string()}),
        };
    }
    let (pid, hwnd) = match apps::resolve_pid_hwnd(arg_u32(args, "pid"), arg_i64(args, "window_id")) { Ok(v) => v, Err(err) => return json!({"ok": false, "error": err}) }; // 解析目标窗
    let (sx1, sy1) = match local_to_screen(hwnd, fx, fy) { Ok(p) => p, Err(err) => return json!({"ok": false, "error": err}) }; // 起点转换
    let (sx2, sy2) = match local_to_screen(hwnd, tx, ty) { Ok(p) => p, Err(err) => return json!({"ok": false, "error": err}) }; // 终点转换
    let prev = focus_for_foreground(hwnd).await; // 拖拽需系统鼠标消息队列
    let res = tokio::task::spawn_blocking(move || input::drag_steps(sx1, sy1, sx2, sy2, &button, duration_ms, steps, &modifiers)).await; // 执行插值拖拽
    restore_foreground(prev); // 还原前台
    match res {
        Ok(Ok(())) => json!({"ok": true, "data": {"pid": pid, "window_id": hwnd, "from": [sx1, sy1], "to": [sx2, sy2], "steps": steps}}),
        Ok(Err(e)) => json!({"ok": false, "error": e.to_string()}),
        Err(e) => json!({"ok": false, "error": e.to_string()}),
    }
}
pub async fn set_value(uia: &UiaHandle, args: &Map<String, Value>) -> Value { // computer_set_value：经 UIA ValuePattern 设值并回读校验
    let Some(value) = args.get("value").and_then(Value::as_str).map(str::to_string) else { return json!({"ok": false, "error": "set_value 缺少必填 string 参数 value"}); }; // 校验 value
    let Some(token) = arg_str(args, "element_token") else { return json!({"ok": false, "error": "set_value 缺少 element_token 参数"}); }; // 校验令牌
    let resolved = match snapshots::resolve_token(&token, arg_u32(args, "pid")) { Ok(r) => r, Err(err) => return json!({"ok": false, "error": err}) }; // 解析缓存元素
    let eid = resolved.element.element_id; // 内部元素 ID
    let expected = value.clone(); // 回读比对副本
    let res = uia.call(move |worker| {
        let el = worker.get(eid)?; // 取元素
        let pat: IUIAutomationValuePattern = unsafe { el.GetCurrentPatternAs(UIA_ValuePatternId) }.map_err(|_| anyhow::anyhow!("元素不支持 ValuePattern，请改用 click + type_text"))?; // 获取模式
        if unsafe { pat.CurrentIsReadOnly() }?.as_bool() { anyhow::bail!("目标元素为只读，无法 set_value"); } // 只读拒绝
        unsafe { pat.SetValue(&BSTR::from(value.as_str())) }?; // 写入新值
        std::thread::sleep(std::time::Duration::from_millis(60)); // 短暂等待控件同步
        let actual = unsafe { pat.CurrentValue() }.map(|s| s.to_string()).unwrap_or_default(); // 回读当前值
        Ok(json!({"element_token": token, "verified": actual == expected, "readback": if resolved.element.is_password { Value::Null } else { json!(actual) }})) // 返回结果
    }).await;
    match res { Ok(data) => json!({"ok": true, "data": data}), Err(e) => json!({"ok": false, "error": e.to_string()}) } // 封装外层格式
}
pub async fn type_text(uia: &UiaHandle, args: &Map<String, Value>) -> Value { // computer_type_text：支持 scope=desktop、element_token(ValuePattern/Focus) 及 x,y 先点后输
    overlay::show(); // 显示覆盖层
    let Some(text) = args.get("text").and_then(Value::as_str).map(str::to_string) else { return json!({"ok": false, "error": "type_text 缺少必填参数 text"}); }; // 文本必填
    let delay_ms = args.get("delay_ms").and_then(Value::as_u64).unwrap_or(0).min(200); // 0..=200ms 间隔
    if is_desktop_scope(args) { // 1) desktop 作用域直接输往当前前台焦点
        let count = text.chars().count(); // 字符数
        return match tokio::task::spawn_blocking(move || input::type_text_multiline(&text, delay_ms)).await {
            Ok(Ok(())) => json!({"ok": true, "data": {"scope": "desktop", "typed_chars": count}}),
            Ok(Err(e)) => json!({"ok": false, "error": e.to_string()}),
            Err(e) => json!({"ok": false, "error": e.to_string()}),
        };
    }
    let pid_opt = arg_u32(args, "pid"); // 可选 pid
    if let Some(token) = arg_str(args, "element_token") { // 2) element_token：优先 UIA ValuePattern（可不抢焦点），不支持时置焦 SendInput
        let resolved = match snapshots::resolve_token(&token, pid_opt) { Ok(r) => r, Err(err) => return json!({"ok": false, "error": err}) }; // 解析令牌
        let eid = resolved.element.element_id; // 元素 ID
        let text_for_uia = text.clone(); // UIA 写入副本
        let uia_try = uia.call(move |worker| {
            let el = worker.get(eid)?; // 取元素
            if let Ok(pat) = unsafe { el.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) } {
                if !unsafe { pat.CurrentIsReadOnly() }?.as_bool() { unsafe { pat.SetValue(&BSTR::from(text_for_uia.as_str())) }?; return Ok(json!({"path": "uia_value"})); } // ValuePattern 写入
            }
            let _ = unsafe { el.SetFocus() }; // 回退先 SetFocus
            Ok(json!({"path": "focus_needed"})) // 需要 SendInput
        }).await;
        if uia_try.as_ref().ok().and_then(|v| v.get("path")).and_then(Value::as_str) == Some("uia_value") {
            return json!({"ok": true, "data": {"path": "uia_value", "element_token": token, "typed_chars": text.chars().count()}}); // 无焦点打扰完成
        }
        let prev = focus_for_foreground(resolved.hwnd).await; // 置焦窗口并用鼠标点元素中心确保焦点命中
        let (cx, cy) = resolved.element.center; // 元素中心
        let el_hwnd = resolved.element.hwnd; let win_hwnd = resolved.hwnd; // 原生句柄兜底
        let chars = text.chars().count(); // 字符数
        let res = tokio::task::spawn_blocking(move || { let _ = input::click(cx, cy, "left", 1); input::type_text_multiline(&text, delay_ms).or_else(|_| input::background_type_text(if el_hwnd != 0 { el_hwnd } else { win_hwnd } as isize, &text)) }).await; // 点击输入，SendInput 失败回退 WM_CHAR
        restore_foreground(prev); // 恢复原前景
        return match res { Ok(Ok(())) => json!({"ok": true, "data": {"path": "focus_type", "element_token": token, "typed_chars": chars}}), Ok(Err(e)) => json!({"ok": false, "error": e.to_string()}), Err(e) => json!({"ok": false, "error": e.to_string()}) };
    }
    let (pid, hwnd) = match apps::resolve_pid_hwnd(pid_opt, arg_i64(args, "window_id")) { Ok(v) => v, Err(err) => return json!({"ok": false, "error": err}) }; // 解析窗口
    let xy_screen = match (arg_f64(args, "x"), arg_f64(args, "y")) { (Some(x), Some(y)) => match local_to_screen(hwnd, x, y) { Ok(pt) => Some(pt), Err(err) => return json!({"ok": false, "error": err}) }, _ => None }; // 可选点击点
    let prev = focus_for_foreground(hwnd).await; // 激活目标窗口
    let chars = text.chars().count(); // 字符数
    let res = tokio::task::spawn_blocking(move || { if let Some((sx, sy)) = xy_screen { input::click(sx, sy, "left", 1)?; std::thread::sleep(std::time::Duration::from_millis(60)); } input::type_text_multiline(&text, delay_ms).or_else(|_| input::background_type_text(hwnd as isize, &text)) }).await; // 点击后逐字输入（失败回退 WM_CHAR）
    restore_foreground(prev); // 还原旧前台
    match res { Ok(Ok(())) => json!({"ok": true, "data": {"pid": pid, "window_id": hwnd, "typed_chars": chars}}), Ok(Err(e)) => json!({"ok": false, "error": e.to_string()}), Err(e) => json!({"ok": false, "error": e.to_string()}) }
}
async fn dispatch_key_tokens(uia: &UiaHandle, args: &Map<String, Value>, tokens: Vec<String>) -> Value { // 共用单键与组合键派发逻辑
    overlay::show(); // 显示覆盖层
    if is_desktop_scope(args) { // 桌面作用域直接发至当前前台
        return match tokio::task::spawn_blocking(move || input::press_keys_slice(&tokens)).await {
            Ok(Ok(())) => json!({"ok": true, "data": {"scope": "desktop", "path": "send_input"}}),
            Ok(Err(e)) => json!({"ok": false, "error": e.to_string()}),
            Err(e) => json!({"ok": false, "error": e.to_string()}),
        };
    }
    let pid_opt = arg_u32(args, "pid"); // 可选 pid
    let (pid, hwnd, focus_eid, focus_pt) = if let Some(token) = arg_str(args, "element_token") { // element_token 定位
        let r = match snapshots::resolve_token(&token, pid_opt) { Ok(v) => v, Err(e) => return json!({"ok": false, "error": e}) }; // 解析令牌
        (r.pid, r.hwnd, Some(r.element.element_id), None) // 返回句柄
    } else {
        let (p, h) = match apps::resolve_pid_hwnd(pid_opt, arg_i64(args, "window_id")) { Ok(v) => v, Err(e) => return json!({"ok": false, "error": e}) }; // 解析窗口
        let pt = match (arg_f64(args, "x"), arg_f64(args, "y")) { (Some(x), Some(y)) => match local_to_screen(h, x, y) { Ok(pt) => Some(pt), Err(e) => return json!({"ok": false, "error": e}) }, _ => None }; // 可选点击点
        (p, h, None, pt) // 返回句柄
    };
    if !is_foreground(args) && tokens.len() == 1 && focus_pt.is_none() { // 无修饰单键优先 PostMessage 后台送达（不抢焦点）
        if let Ok(vk) = input::vk_of(&tokens[0]) {
            if let Some(eid) = focus_eid { let _ = uia.call(move |w| { if let Ok(el) = w.get(eid) { let _ = unsafe { el.SetFocus() }; } Ok(json!({})) }).await; } // 可选 UIA 聚焦
            if input::background_key(hwnd as isize, vk, true).is_ok() && input::background_key(hwnd as isize, vk, false).is_ok() {
                return json!({"ok": true, "data": {"path": "post_message", "pid": pid, "window_id": hwnd, "keys": tokens}}); // 后台送达成功
            }
        }
    }
    let prev = focus_for_foreground(hwnd).await; // 组合键或显式前台短暂置前并用 SendInput 保证系统修饰键生效
    if let Some(eid) = focus_eid { let _ = uia.call(move |w| { if let Ok(el) = w.get(eid) { let _ = unsafe { el.SetFocus() }; } Ok(json!({})) }).await; } // 聚焦元素
    let keys_out = tokens.clone(); // 返回回显
    let res = tokio::task::spawn_blocking(move || { if let Some((sx, sy)) = focus_pt { let _ = input::click(sx, sy, "left", 1); std::thread::sleep(std::time::Duration::from_millis(50)); } input::press_keys_slice(&tokens) }).await; // 发送按键
    restore_foreground(prev); // 恢复旧前台
    match res {
        Ok(Ok(())) => json!({"ok": true, "data": {"path": "send_input", "pid": pid, "window_id": hwnd, "keys": keys_out}}),
        Ok(Err(e)) => json!({"ok": false, "error": e.to_string()}),
        Err(e) => json!({"ok": false, "error": e.to_string()}),
    }
}
pub async fn press_key(uia: &UiaHandle, args: &Map<String, Value>) -> Value { // computer_press_key：单键及可选 modifiers
    let Some(key) = arg_str(args, "key") else { return json!({"ok": false, "error": "press_key 缺少必填参数 key"}); }; // 校验 key
    let mut tokens = arg_strs(args, "modifiers"); // 先放修饰键
    tokens.push(key); // 最后放主键
    dispatch_key_tokens(uia, args, tokens).await // 复用组合键通道
}
pub async fn hotkey(uia: &UiaHandle, args: &Map<String, Value>) -> Value { // computer_hotkey：组合键数组 keys（至少 2 键）
    let keys = arg_strs(args, "keys"); // 读取数组
    if keys.len() < 2 { return json!({"ok": false, "error": "hotkey 的 keys 至少需要 2 个按键"}); } // 校验长度
    dispatch_key_tokens(uia, args, keys).await // 发送组合键
}
pub async fn scroll(uia: &UiaHandle, args: &Map<String, Value>) -> Value { // computer_scroll：支持 desktop、element_token(ScrollPattern/坐标) 与窗口滚动
    overlay::show(); // 显示覆盖层
    let Some(direction) = arg_str(args, "direction") else { return json!({"ok": false, "error": "scroll 缺少必填参数 direction"}); }; // 校验方向
    let by = arg_str(args, "by").unwrap_or_else(|| "line".to_string()); // line | page
    let amount = args.get("amount").and_then(Value::as_i64).unwrap_or(3).clamp(1, 50) as i32; // 默认 3 步
    let fg = is_foreground(args); // 是否强制前台
    if arg_f64(args, "x").is_some() && arg_f64(args, "y").is_some() && args.get("pid").is_none() && args.get("window_id").is_none() && !is_desktop_scope(args) && arg_str(args, "element_token").is_none() {
        return json!({"ok": false, "error": "desktop_coordinate_scope_required: 无 pid/window_id 且传 (x,y) 时必须指定 scope=\"desktop\""}); // 拒绝无域坐标
    }
    if is_desktop_scope(args) { // 1) desktop 绝对坐标滚轮
        let (Some(x), Some(y)) = (arg_f64(args, "x"), arg_f64(args, "y")) else { return json!({"ok": false, "error": "desktop 作用域滚动必须提供 x 与 y"}); }; // 校验 xy
        let (sx, sy) = (x.round() as i32, y.round() as i32); // 屏幕坐标
        let dir_in = direction.clone(); let by_in = by.clone(); // 移入线程副本
        return match tokio::task::spawn_blocking(move || input::scroll_ticks(sx, sy, &dir_in, &by_in, amount)).await {
            Ok(Ok(())) => json!({"ok": true, "data": {"scope": "desktop", "direction": direction, "by": by, "amount": amount}}),
            Ok(Err(e)) => json!({"ok": false, "error": e.to_string()}),
            Err(e) => json!({"ok": false, "error": e.to_string()}),
        };
    }
    let pid_opt = arg_u32(args, "pid"); // 可选 pid
    if let Some(token) = arg_str(args, "element_token") { // 2) element_token：优先 ScrollPattern，不支持则对元素中心发滚轮
        let r = match snapshots::resolve_token(&token, pid_opt) { Ok(v) => v, Err(e) => return json!({"ok": false, "error": e}) }; // 解析令牌
        let eid = r.element.element_id; // 元素 ID
        let dir_c = direction.clone(); let by_c = by.clone(); // 移动副本
        let uia_res = uia.call(move |worker| {
            let el = worker.get(eid)?; // 取元素
            let pat: IUIAutomationScrollPattern = unsafe { el.GetCurrentPatternAs(UIA_ScrollPatternId) }.map_err(|_| anyhow::anyhow!("no_scroll_pattern"))?; // 探测 ScrollPattern
            let large = by_c.eq_ignore_ascii_case("page"); // 是否按页
            let dec = if large { ScrollAmount_LargeDecrement } else { ScrollAmount_SmallDecrement }; // 减量
            let inc = if large { ScrollAmount_LargeIncrement } else { ScrollAmount_SmallIncrement }; // 增量
            let (h_amt, v_amt) = match dir_c.to_ascii_lowercase().as_str() { "up" => (ScrollAmount_NoAmount, dec), "down" => (ScrollAmount_NoAmount, inc), "left" => (dec, ScrollAmount_NoAmount), "right" => (inc, ScrollAmount_NoAmount), _ => (ScrollAmount_NoAmount, ScrollAmount_NoAmount) }; // 映射步长
            for _ in 0..amount { unsafe { pat.Scroll(h_amt, v_amt) }?; } // 执行滚动
            Ok(json!({"path": "ax_scroll"})) // 成功
        }).await;
        if uia_res.is_ok() { return json!({"ok": true, "data": {"path": "ax_scroll", "element_token": token, "direction": direction, "by": by, "amount": amount}}); } // 后台成功
        let (cx, cy) = r.element.center; // 回退物理滚轮到元素中心
        let prev = focus_for_foreground(r.hwnd).await; // 置前
        let res = tokio::task::spawn_blocking(move || input::scroll_ticks(cx, cy, &direction, &by, amount)).await; // 滚动
        restore_foreground(prev); // 恢复
        return match res { Ok(Ok(())) => json!({"ok": true, "data": {"path": "wheel", "element_token": token, "x": cx, "y": cy}}), Ok(Err(e)) => json!({"ok": false, "error": e.to_string()}), Err(e) => json!({"ok": false, "error": e.to_string()}) };
    }
    let (pid, hwnd) = match apps::resolve_pid_hwnd(pid_opt, arg_i64(args, "window_id")) { Ok(v) => v, Err(e) => return json!({"ok": false, "error": e}) }; // 3) 窗口级滚动
    if !fg && args.get("x").is_none() { // 无精确子坐标时先尝试后台 WM_VSCROLL/WM_HSCROLL
        if input::background_scroll(hwnd as isize, &direction, &by, amount).is_ok() {
            return json!({"ok": true, "data": {"path": "post_message", "pid": pid, "window_id": hwnd, "direction": direction, "by": by, "amount": amount}});
        }
    }
    let (sx, sy) = match (arg_f64(args, "x"), arg_f64(args, "y")) { // 解析目标坐标（缺省取窗口中心）
        (Some(x), Some(y)) => match local_to_screen(hwnd, x, y) { Ok(p) => p, Err(e) => return json!({"ok": false, "error": e}) },
        _ => { let (ox, oy, w, h) = screen::window_bounds_outer(hwnd).unwrap_or((0, 0, 200, 200)); (ox + w / 2, oy + h / 2) }
    };
    let prev = focus_for_foreground(hwnd).await; // 激活窗口发送鼠标滚轮
    let res = tokio::task::spawn_blocking(move || input::scroll_ticks(sx, sy, &direction, &by, amount)).await; // 滚轮
    restore_foreground(prev); // 恢复
    match res { Ok(Ok(())) => json!({"ok": true, "data": {"path": "wheel", "pid": pid, "window_id": hwnd, "screen_x": sx, "screen_y": sy}}), Ok(Err(e)) => json!({"ok": false, "error": e.to_string()}), Err(e) => json!({"ok": false, "error": e.to_string()}) }
}
pub fn move_cursor(args: &Map<String, Value>) -> Value { // computer_move_cursor：desktop 移动系统真实指针，window 仅刷新覆盖层提示
    let (Some(x), Some(y)) = (arg_f64(args, "x"), arg_f64(args, "y")) else { return json!({"ok": false, "error": "move_cursor 缺少 x/y 参数"}); }; // 校验坐标
    let (ix, iy) = (x.round() as i32, y.round() as i32); // 取整
    let desktop_mode = arg_str(args, "scope").map(|s| s.eq_ignore_ascii_case("desktop")).unwrap_or(false); // 是否 desktop 作用域
    overlay::show(); // 显示操作覆盖层
    if desktop_mode { // 真实系统指针移动
        if let Err(err) = input::move_to(ix, iy) { return json!({"ok": false, "error": err.to_string()}); } // 移动 OS 指针
        return json!({"ok": true, "data": {"scope": "desktop", "x": ix, "y": iy, "moved_os_cursor": true}}); // 回执
    }
    json!({"ok": true, "data": {"scope": "window", "x": ix, "y": iy, "moved_os_cursor": false}}) // window 仅移动虚拟覆盖态
}
fn norm_menu_name(s: &str) -> String { s.replace('&', "").split('\t').next().unwrap_or(s).trim().to_ascii_lowercase() } // 剥离快捷键符号与助记符后归一化比较
pub async fn invoke_menu(uia: &UiaHandle, args: &Map<String, Value>) -> Value { // computer_invoke_menu：激活窗口并按 path 逐层展开/触发原生 UIA 菜单项
    let Some(pid) = arg_u32(args, "pid") else { return json!({"ok": false, "error": "invoke_menu 缺少 pid"}); }; // 必填 pid
    let Some(hwnd) = arg_i64(args, "window_id") else { return json!({"ok": false, "error": "invoke_menu 缺少 window_id"}); }; // 必填 window_id
    let path = arg_strs(args, "path"); // 菜单路径数组
    if path.is_empty() || path.len() > 16 { return json!({"ok": false, "error": "invoke_menu 的 path 需包含 1..=16 个非空菜单段"}); } // 校验路径
    if let Err(err) = apps::resolve_pid_hwnd(Some(pid), Some(hwnd)) { return json!({"ok": false, "error": err}); } // 校验属主
    let prev = focus_for_foreground(hwnd).await; // 原生菜单要求目标窗处于激活态
    let hops = path.clone(); // 移入 UIA 线程
    let res = uia.call(move |worker| {
        let auto = worker.automation(); // UIA 入口
        let mut scope = worker.element_from_handle(hwnd)?; // 起点为窗口根元素
        let menu_cond = unsafe { auto.CreatePropertyCondition(UIA_ControlTypePropertyId, &windows::Win32::System::Variant::VARIANT::from(UIA_MenuItemControlTypeId.0)) }?; // 匹配 MenuItem
        for (idx, seg) in hops.iter().enumerate() { // 逐层匹配菜单段
            let want = norm_menu_name(seg); // 目标菜单名
            let is_last = idx + 1 == hops.len(); // 是否最后一项
            let mut matched = None; // 当前层匹配元素
            let candidates = unsafe { scope.FindAll(TreeScope_Descendants, &menu_cond) }.ok(); // 查找当前作用域后代 MenuItem
            if let Some(arr) = candidates { // 扫描候选
                let len = unsafe { arr.Length() }.unwrap_or(0); // 个数
                for i in 0..len { // 遍历
                    if let Ok(item) = unsafe { arr.GetElement(i) } { // 取出项
                        let name = unsafe { item.CurrentName() }.map(|s| s.to_string()).unwrap_or_default(); // 读名称
                        let n = norm_menu_name(&name); // 归一化
                        if n == want || n.starts_with(&format!("{want} ")) { matched = Some(item); break; } // 精确匹配或去参数后缀匹配
                    }
                }
            }
            if matched.is_none() && idx == 0 { // 兜底查独立 MenuBar / 系统菜单弹出窗
                let root_desktop = unsafe { auto.GetRootElement() }?; // 桌面根
                let mbar_cond = unsafe { auto.CreatePropertyCondition(UIA_ControlTypePropertyId, &windows::Win32::System::Variant::VARIANT::from(UIA_MenuBarControlTypeId.0)) }?; // MenuBar 条件
                if let Ok(mbars) = unsafe { scope.FindAll(TreeScope_Descendants, &mbar_cond) } { let _ = (root_desktop, mbars); } // 保留扩展点
            }
            let target = matched.ok_or_else(|| anyhow::anyhow!("menu_path_unavailable: 未找到菜单项 {seg}"))?; // 未命中则终止
            if is_last { // 叶子节点执行 Invoke
                let pat: IUIAutomationInvokePattern = unsafe { target.GetCurrentPatternAs(UIA_InvokePatternId) }.map_err(|_| anyhow::anyhow!("菜单项 {seg} 不支持 InvokePattern"))?; // 获取 Invoke
                unsafe { pat.Invoke() }?; // 触发菜单
                break; // 完成
            }
            if let Ok(pat) = unsafe { target.GetCurrentPatternAs::<IUIAutomationExpandCollapsePattern>(UIA_ExpandCollapsePatternId) } { unsafe { pat.Expand() }?; } // 非末节点先 Expand
            else if let Ok(pat) = unsafe { target.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId) } { unsafe { pat.Invoke() }?; } // 无 Expand 则 Invoke 弹出
            std::thread::sleep(std::time::Duration::from_millis(120)); // 等待子菜单挂载进 UIA 树
            let popup_cond = unsafe { auto.CreatePropertyCondition(UIA_ControlTypePropertyId, &windows::Win32::System::Variant::VARIANT::from(UIA_MenuControlTypeId.0)) }?; // 查找弹出 Menu
            let root_desktop = unsafe { auto.GetRootElement() }?; // 桌面根（弹出菜单常挂载为顶层窗）
            scope = unsafe { target.FindFirst(TreeScope_Descendants, &popup_cond) }.ok().or_else(|| unsafe { root_desktop.FindFirst(TreeScope_Descendants, &popup_cond) }.ok()).unwrap_or(target); // 更新下一跳搜索域
        }
        Ok(json!({"pid": pid, "window_id": hwnd, "invoked_path": hops})) // 成功返回
    }).await;
    restore_foreground(prev); // 恢复原前台
    match res { Ok(data) => json!({"ok": true, "data": data}), Err(e) => json!({"ok": false, "error": e.to_string()}) } // 返回封装
}