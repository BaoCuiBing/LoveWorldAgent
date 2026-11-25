use windows::Win32::System::Com::{GetRunningObjectTable, IRunningObjectTable}; // ROT 接口
use windows::core::GUID; // GUID 类型
use std::sync::{Mutex, OnceLock}; // 受控实例注册表
use std::cell::RefCell; // STA 线程内受控 COM 实例
use crate::com::constants::WORD_PROGID; // 默认 Word ProgID
use crate::com::dispatch::{attach_app, create_app, get, set, ComObj, ComResult, Variant}; // COM 工具
use crate::word::report::ToolError; // 错误类型
static CONTROLLED: OnceLock<Mutex<Vec<i32>>> = OnceLock::new(); // 本 sidecar 创建/接管的窗口句柄
thread_local! { static CONTROLLED_APPS: RefCell<Vec<ComObj>> = RefCell::new(Vec::new()); } // STA 线程内受控 Application（quit 用）
fn controlled_list() -> &'static Mutex<Vec<i32>> { CONTROLLED.get_or_init(|| Mutex::new(Vec::new())) } // 懒初始化
pub fn register_controlled(hwnd: i32) { // 登记受控实例
    if hwnd == 0 { return; } // 无窗口跳过
    if let Ok(mut list) = controlled_list().lock() { if !list.contains(&hwnd) { list.push(hwnd); } } // 去重加入
}
pub fn register_controlled_app(app: ComObj) { // 登记受控 Application 与窗口
    if let Some(hwnd) = app_hwnd(&app) { register_controlled(hwnd); } // 窗口登记
    CONTROLLED_APPS.with(|apps| { let mut apps = apps.borrow_mut(); if !apps.iter().any(|item| std::ptr::eq(item.raw(), app.raw())) { apps.push(app); } }); // COM 引用登记（去重）
}
pub fn take_controlled_apps() -> Vec<ComObj> { CONTROLLED_APPS.with(|apps| apps.borrow_mut().drain(..).collect()) } // 取出并清空
fn app_hwnd(app: &ComObj) -> Option<i32> { // 读活动窗口句柄
    get(app, "ActiveWindow").ok()?.as_dispatch().ok().and_then(|window| get(&window, "Hwnd").ok()).map(|value| value.as_i32()) // 链式读取
}
pub fn controlled_hwnds() -> Vec<i32> { controlled_list().lock().map(|list| list.clone()).unwrap_or_default() } // 快照
pub fn clear_controlled() { if let Ok(mut list) = controlled_list().lock() { list.clear(); } } // 清空
pub struct AppHandle { // 受控 Office 应用
    pub com: ComObj, // Application 对象
    pub controlled: bool, // 是否本 sidecar 创建/接管
    pub progid: &'static str, // 族 ProgID
}
impl AppHandle { // 应用操作
    pub fn create_with(progid: &'static str) -> ComResult<Self> { // 新建指定族可见实例
        let com = create_app(progid)?; // 创建
        let _ = set(&com, "Visible", Variant::Bool(true)); // 可见（各族尽力）
        let _ = set(&com, "DisplayAlerts", Variant::I4(0)); // 静默
        let _ = set(&com, "AutomationSecurity", Variant::I4(3)); // 禁用宏（Word 专用，其余忽略）
        Ok(Self { com, controlled: true, progid }) // 返回受控实例
    }
    pub fn create_new() -> ComResult<Self> { Self::create_with(WORD_PROGID) } // Word 兼容入口
    pub fn attach_with(progid: &'static str) -> ComResult<Self> { // 附着指定族实例
        let com = attach_app(progid)?; // 附着
        Ok(Self { com, controlled: false, progid }) // 返回非受控
    }
    pub fn attach_existing() -> ComResult<Self> { Self::attach_with(WORD_PROGID) } // Word 兼容入口
    pub fn set_visible(&self, visible: bool) -> ComResult<()> { // 设置可见性
        set(&self.com, "Visible", Variant::Bool(visible)) // 委托
    }
    pub fn ready(&self) -> bool { // 是否就绪
        get(&self.com, "Ready").map(|v| v.as_bool()).unwrap_or(false) // 读 Ready
    }
    pub fn docs_count(&self) -> i32 { // 打开文档数
        get(&self.com, "Documents").and_then(|docs| get(&docs.as_dispatch()?, "Count")).map(|v| v.as_i32()).unwrap_or(0) // 读 Count
    }
    pub fn active_window_hwnd(&self) -> Option<i32> { // 活动窗口句柄
        let window = get(&self.com, "ActiveWindow").ok()?.as_dispatch().ok()?; // 取窗口
        get(&window, "Hwnd").ok().map(|v| v.as_i32()) // 读 Hwnd
    }
    pub fn quit(&self) -> ComResult<()> { // 退出应用
        crate::com::dispatch::call(&self.com, "Quit", &[]) .map(|_| ()) // 调用 Quit
    }
}
pub fn running_object_table() -> ComResult<IRunningObjectTable> { // 取 ROT
    unsafe { GetRunningObjectTable(0) }.map_err(|e| crate::com::dispatch::map_hresult(&e, "GetRunningObjectTable")) // 映射错误
}
pub fn word_clsid() -> ComResult<GUID> { // 暴露 CLSID
    crate::com::dispatch::word_clsid() // 委托
}
pub fn no_word_error() -> ToolError { // 无 Word 错误
    ToolError::new("NO_WORD", "没有运行中的 Word 实例") // 构造
}
