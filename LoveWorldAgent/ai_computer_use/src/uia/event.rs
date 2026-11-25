use anyhow::{anyhow, bail, Result}; // 错误类型
use windows::Win32::Foundation::HWND; // 窗口句柄
use windows::core::BSTR; // 宽字符串
use windows::Win32::UI::Accessibility::{IUIAutomationDockPattern, IUIAutomationExpandCollapsePattern, IUIAutomationInvokePattern, IUIAutomationMultipleViewPattern, IUIAutomationRangeValuePattern, IUIAutomationScrollItemPattern, IUIAutomationScrollPattern, IUIAutomationSelectionItemPattern, IUIAutomationTextPattern, IUIAutomationTogglePattern, IUIAutomationTransformPattern, IUIAutomationValuePattern, TextPatternRangeEndpoint_End, TextPatternRangeEndpoint_Start, TextUnit_Character, DockPosition_Bottom, DockPosition_Fill, DockPosition_Left, DockPosition_None, DockPosition_Right, DockPosition_Top, ExpandCollapseState_Collapsed, ExpandCollapseState_Expanded, ExpandCollapseState_PartiallyExpanded, ScrollAmount_LargeDecrement, ScrollAmount_LargeIncrement, ScrollAmount_NoAmount, UIA_DockPatternId, UIA_ExpandCollapsePatternId, UIA_InvokePatternId, UIA_MultipleViewPatternId, UIA_RangeValuePatternId, UIA_ScrollItemPatternId, UIA_ScrollPatternId, UIA_SelectionItemPatternId, UIA_TextPatternId, UIA_TogglePatternId, UIA_TransformPatternId, UIA_ValuePatternId}; // UIA 模式
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, ShowWindow, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, WM_CLOSE}; // 窗口动作
use crate::uia::element::element_info; // 元素属性
use crate::uia::input; // 输入能力
use crate::uia::worker::UiaWorker; // UIA 工作器
#[allow(dead_code)] // 动作清单供文档与提示词引用
pub const ACTIONS: [(&str, &str); 8] = [ // 严格对齐内置 8 类语义动作
    ("invoke", "触发元素默认操作"), // 调用
    ("toggle", "切换元素状态"), // 切换
    ("select", "选择元素"), // 选择
    ("expand", "展开元素"), // 展开
    ("collapse", "折叠元素"), // 折叠
    ("set_value", "设置元素值"), // 设值
    ("set_focus", "把键盘焦点移到元素"), // 聚焦
    ("scroll_into_view", "把元素滚动到可见区域"), // 滚动到可见
];
#[derive(Debug, Clone, Default)] // 动作请求
pub struct ActionRequest { // 对应 computer_perform_action 的元素动作
    pub element_id: i64, // 目标元素 ID
    pub action: String, // 动作名称
    pub value: Option<String>, // 动作参数
    pub x: Option<f64>, // 坐标 X
    pub y: Option<f64>, // 坐标 Y
    pub width: Option<f64>, // 宽度
    pub height: Option<f64>, // 高度
    pub position: Option<String>, // 停靠位置或视图参数
    pub amount: Option<f64>, // 滚动比例 0~100
    pub view_id: Option<i32>, // 视图 ID
}
pub fn invoke_action(worker: &mut UiaWorker, request: &ActionRequest) -> Result<String> { // 执行内置 6 类 UIA 语义动作
    let element = worker.get(request.element_id)?; // 取目标元素
    let info = element_info(request.element_id, &element); // 读取属性（含原生句柄）
    match request.action.as_str() { // 按动作分派
        "invoke" => { let pattern: IUIAutomationInvokePattern = unsafe { element.GetCurrentPatternAs(UIA_InvokePatternId) }.map_err(|_| anyhow!("元素不支持 Invoke"))?; unsafe { pattern.Invoke() }?; Ok(format!("已触发 {}", info.name)) } // 调用（UIA 无回执）
        "toggle" => { let pattern: IUIAutomationTogglePattern = unsafe { element.GetCurrentPatternAs(UIA_TogglePatternId) }.map_err(|_| anyhow!("元素不支持 Toggle"))?; let before = unsafe { pattern.CurrentToggleState() }?; unsafe { pattern.Toggle() }?; std::thread::sleep(std::time::Duration::from_millis(60)); let after = unsafe { pattern.CurrentToggleState() }?; if before == after { bail!("toggle 未生效：状态仍为切换前"); } Ok(format!("已切换 {}", info.name)) } // 切换并校验
        "select" => { let pattern: IUIAutomationSelectionItemPattern = unsafe { element.GetCurrentPatternAs(UIA_SelectionItemPatternId) }.map_err(|_| anyhow!("元素不支持 SelectionItem"))?; unsafe { pattern.Select() }?; std::thread::sleep(std::time::Duration::from_millis(60)); if !unsafe { pattern.CurrentIsSelected() }?.as_bool() { bail!("select 未生效：元素未被选中"); } Ok(format!("已选中 {}", info.name)) } // 选择并校验
        "expand" => { let pattern: IUIAutomationExpandCollapsePattern = unsafe { element.GetCurrentPatternAs(UIA_ExpandCollapsePatternId) }.map_err(|_| anyhow!("元素不支持 ExpandCollapse"))?; unsafe { pattern.Expand() }?; std::thread::sleep(std::time::Duration::from_millis(80)); let state = unsafe { pattern.CurrentExpandCollapseState() }?; if state != ExpandCollapseState_Expanded && state != ExpandCollapseState_PartiallyExpanded { bail!("expand 未生效：当前状态不是展开"); } Ok(format!("已展开 {}", info.name)) } // 展开并校验
        "collapse" => { let pattern: IUIAutomationExpandCollapsePattern = unsafe { element.GetCurrentPatternAs(UIA_ExpandCollapsePatternId) }.map_err(|_| anyhow!("元素不支持 ExpandCollapse"))?; unsafe { pattern.Collapse() }?; std::thread::sleep(std::time::Duration::from_millis(80)); if unsafe { pattern.CurrentExpandCollapseState() }? != ExpandCollapseState_Collapsed { bail!("collapse 未生效：当前状态不是折叠"); } Ok(format!("已折叠 {}", info.name)) } // 折叠并校验
        "set_value" => { let pattern: IUIAutomationValuePattern = unsafe { element.GetCurrentPatternAs(UIA_ValuePatternId) }.map_err(|_| anyhow!("元素不支持 ValuePattern"))?; if unsafe { pattern.CurrentIsReadOnly() }?.as_bool() { bail!("元素为只读，无法 set_value"); } let value = request.value.clone().unwrap_or_default(); unsafe { pattern.SetValue(&BSTR::from(value.as_str())) }?; std::thread::sleep(std::time::Duration::from_millis(60)); let actual = unsafe { pattern.CurrentValue() }?.to_string(); if actual != value { bail!("set_value 未生效：写入后回读为 \"{actual}\"（部分应用会拒绝 ValuePattern，请改用 click + type_text）"); } Ok(format!("已设置 {}", info.name)) } // 设值并回读校验（A1）
        "set_focus" => { unsafe { element.SetFocus() }?; std::thread::sleep(std::time::Duration::from_millis(60)); if !unsafe { element.CurrentHasKeyboardFocus() }.map(|value| value.as_bool()).unwrap_or(false) { bail!("set_focus 未生效：焦点未落在元素上"); } Ok(format!("已聚焦 {}", info.name)) } // 聚焦并校验
        "scroll_into_view" => { let pattern: IUIAutomationScrollItemPattern = unsafe { element.GetCurrentPatternAs(UIA_ScrollItemPatternId) }.map_err(|_| anyhow!("元素不支持 ScrollItem"))?; unsafe { pattern.ScrollIntoView() }?; std::thread::sleep(std::time::Duration::from_millis(60)); if unsafe { element.CurrentIsOffscreen() }.map(|value| value.as_bool()).unwrap_or(false) { bail!("scroll_into_view 未生效：元素仍处于离屏状态"); } Ok(format!("已滚动到可见 {}", info.name)) } // 滚动到可见并校验
        other => bail!("不支持的动作: {other}"), // 未知动作
    }
}
pub fn click_at(x: i32, y: i32, button: &str, clicks: usize) -> Result<String> { input::click(x, y, button, clicks)?; Ok(format!("已{button}键点击 ({x},{y})")) } // 坐标点击
pub fn type_text(text: &str) -> Result<String> { input::type_text(text)?; Ok(format!("已输入 {} 个字符", text.chars().count())) } // 前台文本输入
pub fn press_key(key: &str) -> Result<String> { input::press_key(key)?; Ok(format!("已发送按键 {key}")) } // 组合键
pub fn scroll(x: i32, y: i32, delta: i32) -> Result<String> { input::scroll(x, y, delta)?; Ok(format!("已在 ({x},{y}) 滚动 {delta}")) } // 滚轮
pub fn move_mouse(x: i32, y: i32) -> Result<String> { input::move_to(x, y)?; Ok(format!("已移动到 ({x},{y})")) } // 移动鼠标
pub fn focus_element(worker: &mut UiaWorker, element_id: i64) -> Result<()> { let element = worker.get(element_id)?; unsafe { element.SetFocus()?; } Ok(()) } // 聚焦元素
pub fn scroll_element(worker: &mut UiaWorker, element_id: i64, direction: &str, pages: f64) -> Result<String> { // 语义滚动（内置 scroll 的 element_id 路径）
    let element = worker.get(element_id)?; // 目标元素
    let pattern: IUIAutomationScrollPattern = unsafe { element.GetCurrentPatternAs(UIA_ScrollPatternId) }.map_err(|_| anyhow!("元素不支持 Scroll"))?; // ScrollPattern
    let times = (pages.ceil() as usize).clamp(1, 50); // 页数向上取整，上限 50
    let amount = if direction == "up" || direction == "left" { ScrollAmount_LargeDecrement } else { ScrollAmount_LargeIncrement }; // 方向
    for _ in 0..times { // 逐页滚动
        if direction == "left" || direction == "right" { unsafe { pattern.Scroll(amount, ScrollAmount_NoAmount) }?; } else { unsafe { pattern.Scroll(ScrollAmount_NoAmount, amount) }?; } // 水平/垂直
    }
    Ok(format!("已滚动 {direction} {times} 页")) // 返回
}
pub fn select_text_range(worker: &mut UiaWorker, element_id: i64, text: &str, prefix: &str, suffix: &str, selection: &str) -> Result<(usize, usize)> { // TextPattern 选区（prefix/suffix 消歧）
    if text.is_empty() { bail!("text 不能为空"); } // 空文本
    let element = worker.get(element_id)?; // 目标元素
    let pattern: IUIAutomationTextPattern = unsafe { element.GetCurrentPatternAs(UIA_TextPatternId) }.map_err(|err| anyhow!("元素不支持 TextPattern: {err}"))?; // 取文本模式
    let document = unsafe { pattern.DocumentRange()? }; // 文档范围
    let needle = format!("{prefix}{text}{suffix}"); // 带上下文的目标
    let range = unsafe { document.FindText(&BSTR::from(needle.as_str()), false, true) }.map_err(|err| anyhow!("未找到目标文本: {err}"))?; // 查找文本
    if !prefix.is_empty() { let moved = unsafe { range.MoveEndpointByUnit(TextPatternRangeEndpoint_Start, TextUnit_Character, prefix.chars().count() as i32)? }; if moved == 0 { bail!("无法将选区起点移到目标文本"); } } // 去掉前缀
    if !suffix.is_empty() { let moved = unsafe { range.MoveEndpointByUnit(TextPatternRangeEndpoint_End, TextUnit_Character, -(suffix.chars().count() as i32))? }; if moved == 0 { bail!("无法将选区终点移到目标文本"); } } // 去掉后缀
    match selection { // 选区方式
        "cursor_before" => { let collapsed = unsafe { range.Clone()? }; unsafe { collapsed.MoveEndpointByRange(TextPatternRangeEndpoint_End, &collapsed, TextPatternRangeEndpoint_Start)?; collapsed.Select()?; } } // 光标在文本前
        "cursor_after" => { let collapsed = unsafe { range.Clone()? }; unsafe { collapsed.MoveEndpointByRange(TextPatternRangeEndpoint_Start, &collapsed, TextPatternRangeEndpoint_End)?; collapsed.Select()?; } } // 光标在文本后
        _ => unsafe { range.Select()?; }, // 选中文本
    }
    Ok((prefix.chars().count(), prefix.chars().count() + text.chars().count())) // 返回目标文本在 needle 中的区间
}
