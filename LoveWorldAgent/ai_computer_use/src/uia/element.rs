use serde::Serialize; // 序列化派生
use windows::Win32::Foundation::RECT; // 矩形类型
use windows::Win32::UI::Accessibility::{IUIAutomationElement, IUIAutomationDockPattern, IUIAutomationExpandCollapsePattern, IUIAutomationGridItemPattern, IUIAutomationGridPattern, IUIAutomationInvokePattern, IUIAutomationMultipleViewPattern, IUIAutomationRangeValuePattern, IUIAutomationScrollItemPattern, IUIAutomationScrollPattern, IUIAutomationSelectionItemPattern, IUIAutomationSelectionPattern, IUIAutomationTableItemPattern, IUIAutomationTablePattern, IUIAutomationTextPattern, IUIAutomationTogglePattern, IUIAutomationTransformPattern, IUIAutomationValuePattern, IUIAutomationWindowPattern, ExpandCollapseState_Collapsed, ExpandCollapseState_Expanded, ExpandCollapseState_PartiallyExpanded, ToggleState_Indeterminate, ToggleState_Off, ToggleState_On, UIA_DockPatternId, UIA_ExpandCollapsePatternId, UIA_GridItemPatternId, UIA_GridPatternId, UIA_InvokePatternId, UIA_MultipleViewPatternId, UIA_RangeValuePatternId, UIA_ScrollItemPatternId, UIA_ScrollPatternId, UIA_SelectionItemPatternId, UIA_SelectionPatternId, UIA_TableItemPatternId, UIA_TablePatternId, UIA_TextPatternId, UIA_TogglePatternId, UIA_TransformPatternId, UIA_ValuePatternId, UIA_WindowPatternId}; // UIA 元素与 Pattern
#[derive(Debug, Clone, Copy, Default, Serialize)] // 矩形信息
pub struct RectInfo { // 元素包围盒
    pub x: i32, // 左坐标
    pub y: i32, // 上坐标
    pub width: i32, // 宽度
    pub height: i32, // 高度
}
#[derive(Debug, Clone, Serialize)] // 元素信息
pub struct ElementInfo { // 元素属性快照
    pub element_id: i64, // 内部元素 ID（0 表示未缓存）
    pub name: String, // Name 属性
    pub automation_id: String, // AutomationId 属性
    pub class_name: String, // ClassName 属性
    pub control_type: String, // ControlType 名称
    pub control_type_id: i32, // ControlType 数值
    pub process_id: u32, // 所属进程
    pub native_window_handle: i64, // 原生窗口句柄
    pub is_enabled: bool, // 是否可用
    pub is_offscreen: bool, // 是否离屏
    pub rect: RectInfo, // 包围盒
    #[serde(skip_serializing_if = "Vec::is_empty")] // 默认树不输出 Pattern 列表
    pub supported_patterns: Vec<String>, // 支持的 Pattern 名称
    #[serde(skip_serializing_if = "Vec::is_empty")] // 动作列表按需填充
    pub actions: Vec<String>, // 可用语义动作（对齐 Trae 树内 actions）
}
pub fn element_info(element_id: i64, element: &IUIAutomationElement) -> ElementInfo { // 读取元素属性
    let rect = unsafe { element.CurrentBoundingRectangle() }.unwrap_or(RECT { left: 0, top: 0, right: 0, bottom: 0 }); // 包围盒，失败回退零值
    let control_type_id = unsafe { element.CurrentControlType() }.map(|value| value.0).unwrap_or(0); // 控件类型数值
    ElementInfo { // 构造属性快照
        element_id, // 内部 ID
        name: unsafe { element.CurrentName() }.map(|value| value.to_string()).unwrap_or_default(), // 名称
        automation_id: unsafe { element.CurrentAutomationId() }.map(|value| value.to_string()).unwrap_or_default(), // 自动化 ID
        class_name: unsafe { element.CurrentClassName() }.map(|value| value.to_string()).unwrap_or_default(), // 类名
        control_type: control_type_name(control_type_id).to_string(), // 控件类型名称
        control_type_id, // 控件类型数值
        process_id: unsafe { element.CurrentProcessId() }.map(|value| value as u32).unwrap_or(0), // 进程 ID
        native_window_handle: unsafe { element.CurrentNativeWindowHandle() }.map(|value| value.0 as isize as i64).unwrap_or(0), // 原生句柄
        is_enabled: unsafe { element.CurrentIsEnabled() }.map(|value| value.as_bool()).unwrap_or(true), // 可用状态
        is_offscreen: unsafe { element.CurrentIsOffscreen() }.map(|value| value.as_bool()).unwrap_or(false), // 离屏状态
        rect: RectInfo { x: rect.left, y: rect.top, width: rect.right - rect.left, height: rect.bottom - rect.top }, // 包围盒
        supported_patterns: Vec::new(), // 默认不探测，按需由 supported_patterns() 填充
        actions: Vec::new(), // 默认不探测，由 element_actions() 填充
    }
}
pub fn element_actions(element: &IUIAutomationElement) -> Vec<String> { // 探测元素可用语义动作（对齐内置 8 类）
    let mut actions: Vec<String> = Vec::new(); // 结果容器
    macro_rules! probe { ($id:expr, $ty:ty, [$($name:literal),+]) => { if unsafe { element.GetCurrentPatternAs::<$ty>($id) }.is_ok() { $(actions.push($name.to_string());)+ } }; } // 探测宏
    probe!(UIA_InvokePatternId, IUIAutomationInvokePattern, ["invoke"]); // 调用
    probe!(UIA_TogglePatternId, IUIAutomationTogglePattern, ["toggle"]); // 切换
    probe!(UIA_SelectionItemPatternId, IUIAutomationSelectionItemPattern, ["select"]); // 选择
    probe!(UIA_ExpandCollapsePatternId, IUIAutomationExpandCollapsePattern, ["expand", "collapse"]); // 展开/折叠
    probe!(UIA_ScrollItemPatternId, IUIAutomationScrollItemPattern, ["scroll_into_view"]); // 滚动到可见
    actions.push("set_focus".to_string()); // 所有元素均可尝试聚焦
    if let Ok(pattern) = unsafe { element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) } { if !unsafe { pattern.CurrentIsReadOnly() }.map(|value| value.as_bool()).unwrap_or(false) { actions.push("set_value".to_string()); } } // 可写值
    actions // 返回动作列表
}
pub fn element_traits(element: &IUIAutomationElement) -> Vec<String> { // 读取内置树节点状态
    let mut traits: Vec<String> = Vec::new(); // 状态容器
    if unsafe { element.CurrentHasKeyboardFocus() }.map(|value| value.as_bool()).unwrap_or(false) { traits.push("focused".to_string()); } // 焦点
    if !unsafe { element.CurrentIsEnabled() }.map(|value| value.as_bool()).unwrap_or(true) { traits.push("disabled".to_string()); } // 禁用
    if unsafe { element.CurrentIsOffscreen() }.map(|value| value.as_bool()).unwrap_or(false) { traits.push("offscreen".to_string()); } // 离屏
    if let Ok(pattern) = unsafe { element.GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(UIA_SelectionItemPatternId) } { if unsafe { pattern.CurrentIsSelected() }.map(|value| value.as_bool()).unwrap_or(false) { traits.push("selected".to_string()); } } // 选中
    if let Ok(pattern) = unsafe { element.GetCurrentPatternAs::<IUIAutomationExpandCollapsePattern>(UIA_ExpandCollapsePatternId) } { if let Ok(state) = unsafe { pattern.CurrentExpandCollapseState() } { if state == ExpandCollapseState_Expanded { traits.push("expanded".to_string()); } else if state == ExpandCollapseState_Collapsed { traits.push("collapsed".to_string()); } else if state == ExpandCollapseState_PartiallyExpanded { traits.push("expanded".to_string()); } } } // 展开/折叠
    traits // 返回状态列表
}
pub fn element_value(element: &IUIAutomationElement) -> Option<String> { // 读取内置树的 val= 字段
    if let Ok(pattern) = unsafe { element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) } { if let Ok(value) = unsafe { pattern.CurrentValue() } { return Some(value.to_string()); } } // 编辑框值
    if let Ok(pattern) = unsafe { element.GetCurrentPatternAs::<IUIAutomationTogglePattern>(UIA_TogglePatternId) } { if let Ok(state) = unsafe { pattern.CurrentToggleState() } { let text = if state == ToggleState_On { "on" } else if state == ToggleState_Off { "off" } else if state == ToggleState_Indeterminate { "mixed" } else { "unknown" }; return Some(text.to_string()); } } // 复选状态
    if let Ok(pattern) = unsafe { element.GetCurrentPatternAs::<IUIAutomationRangeValuePattern>(UIA_RangeValuePatternId) } { if let Ok(value) = unsafe { pattern.CurrentValue() } { return Some(format!("{}", value)); } } // 范围值
    None // 无值
}
pub fn role_name(control_type_id: i32) -> &'static str { // ControlType 数值转内置 role 名称
    match control_type_id { // 对应 UIA_CONTROLTYPE_ID
        50000 => "button", // 按钮
        50001 => "calendar", // 日历
        50002 => "check-box", // 复选框
        50003 => "combo-box", // 组合框
        50004 => "text-field", // 编辑框
        50005 => "link", // 超链接
        50006 => "image", // 图像
        50007 => "list-item", // 列表项
        50008 => "list", // 列表
        50009 => "menu", // 菜单
        50010 => "menu-bar", // 菜单栏
        50011 => "menu-item", // 菜单项
        50012 => "progress-bar", // 进度条
        50013 => "radio-button", // 单选按钮
        50014 => "scroll-bar", // 滚动条
        50015 => "slider", // 滑块
        50016 => "spinner", // 数值调节器
        50017 => "status-bar", // 状态栏
        50018 => "tab-group", // 选项卡组
        50019 => "tab", // 选项卡
        50020 => "label", // 文本
        50021 => "tool-bar", // 工具栏
        50022 => "tool-tip", // 提示
        50023 => "tree", // 树
        50024 => "tree-item", // 树节点
        50025 => "custom", // 自定义
        50026 => "group", // 分组
        50027 => "thumb", // 缩略图
        50028 => "grid", // 数据网格
        50029 => "grid-item", // 数据项
        50030 => "document", // 文档
        50031 => "split-button", // 拆分按钮
        50032 => "window", // 窗口
        50033 => "pane", // 面板
        50034 => "header", // 表头
        50035 => "column-header", // 表头项
        50036 => "table", // 表格
        50037 => "title-bar", // 标题栏
        50038 => "separator", // 分隔符
        50039 => "semantic-zoom", // 语义缩放
        50040 => "app-bar", // 应用栏
        _ => "unknown", // 未知类型
    }
}
pub fn supported_patterns(element: &IUIAutomationElement) -> Vec<String> { // 探测 18 种 UIA Pattern
    let mut patterns: Vec<String> = Vec::new(); // 结果容器
    macro_rules! probe { ($name:literal, $id:expr, $ty:ty) => { if unsafe { element.GetCurrentPatternAs::<$ty>($id) }.is_ok() { patterns.push($name.to_string()); } }; } // 探测宏
    probe!("Invoke", UIA_InvokePatternId, IUIAutomationInvokePattern); // 调用
    probe!("Toggle", UIA_TogglePatternId, IUIAutomationTogglePattern); // 切换
    probe!("ExpandCollapse", UIA_ExpandCollapsePatternId, IUIAutomationExpandCollapsePattern); // 展开折叠
    probe!("Value", UIA_ValuePatternId, IUIAutomationValuePattern); // 值
    probe!("RangeValue", UIA_RangeValuePatternId, IUIAutomationRangeValuePattern); // 范围值
    probe!("Selection", UIA_SelectionPatternId, IUIAutomationSelectionPattern); // 选择容器
    probe!("SelectionItem", UIA_SelectionItemPatternId, IUIAutomationSelectionItemPattern); // 选择项
    probe!("Scroll", UIA_ScrollPatternId, IUIAutomationScrollPattern); // 滚动
    probe!("ScrollItem", UIA_ScrollItemPatternId, IUIAutomationScrollItemPattern); // 滚动项
    probe!("Window", UIA_WindowPatternId, IUIAutomationWindowPattern); // 窗口
    probe!("Transform", UIA_TransformPatternId, IUIAutomationTransformPattern); // 变换
    probe!("Dock", UIA_DockPatternId, IUIAutomationDockPattern); // 停靠
    probe!("Grid", UIA_GridPatternId, IUIAutomationGridPattern); // 网格
    probe!("GridItem", UIA_GridItemPatternId, IUIAutomationGridItemPattern); // 网格项
    probe!("MultipleView", UIA_MultipleViewPatternId, IUIAutomationMultipleViewPattern); // 多视图
    probe!("Text", UIA_TextPatternId, IUIAutomationTextPattern); // 文本
    probe!("Table", UIA_TablePatternId, IUIAutomationTablePattern); // 表格
    probe!("TableItem", UIA_TableItemPatternId, IUIAutomationTableItemPattern); // 表格项
    patterns // 返回命中列表
}
pub fn control_type_name(control_type_id: i32) -> &'static str { // ControlType 数值转名称
    match control_type_id { // 对应 UIA_CONTROLTYPE_ID
        50000 => "Button", // 按钮
        50001 => "Calendar", // 日历
        50002 => "CheckBox", // 复选框
        50003 => "ComboBox", // 组合框
        50004 => "Edit", // 编辑框
        50005 => "Hyperlink", // 超链接
        50006 => "Image", // 图像
        50007 => "ListItem", // 列表项
        50008 => "List", // 列表
        50009 => "Menu", // 菜单
        50010 => "MenuBar", // 菜单栏
        50011 => "MenuItem", // 菜单项
        50012 => "ProgressBar", // 进度条
        50013 => "RadioButton", // 单选按钮
        50014 => "ScrollBar", // 滚动条
        50015 => "Slider", // 滑块
        50016 => "Spinner", // 数值调节器
        50017 => "StatusBar", // 状态栏
        50018 => "Tab", // 选项卡
        50019 => "TabItem", // 选项卡项
        50020 => "Text", // 文本
        50021 => "ToolBar", // 工具栏
        50022 => "ToolTip", // 提示
        50023 => "Tree", // 树
        50024 => "TreeItem", // 树节点
        50025 => "Custom", // 自定义
        50026 => "Group", // 分组
        50027 => "Thumb", // 缩略图
        50028 => "DataGrid", // 数据网格
        50029 => "DataItem", // 数据项
        50030 => "Document", // 文档
        50031 => "SplitButton", // 拆分按钮
        50032 => "Window", // 窗口
        50033 => "Pane", // 面板
        50034 => "Header", // 表头
        50035 => "HeaderItem", // 表头项
        50036 => "Table", // 表格
        50037 => "TitleBar", // 标题栏
        50038 => "Separator", // 分隔符
        50039 => "SemanticZoom", // 语义缩放
        50040 => "AppBar", // 应用栏
        _ => "Unknown", // 未知类型
    }
}
