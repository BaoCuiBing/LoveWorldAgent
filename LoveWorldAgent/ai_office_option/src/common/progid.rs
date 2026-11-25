use std::cell::RefCell; // STA 线程内受控实例
use std::sync::{Mutex, OnceLock}; // 跨线程受控登记
use crate::com::dispatch::{app_clsid, attach_app, create_app, get, set, ComObj, ComResult, Variant}; // COM 工具
use crate::word::report::ToolError; // 错误类型

thread_local! { static CONTROLLED: RefCell<Vec<ComObj>> = RefCell::new(Vec::new()); } // 本线程创建的 Office 实例
static CONTROLLED_LABELS: OnceLock<Mutex<Vec<&'static str>>> = OnceLock::new(); // 受控实例族标记

pub fn installed(progid: &str) -> bool { app_clsid(progid).is_ok() } // ProgID 是否注册（Office 是否安装）

pub fn ensure_app(progid: &'static str, visible: bool) -> ComResult<ComObj> { // 附着已有实例，否则创建
    match attach_app(progid) { // 先尝试附着
        Ok(app) => { if visible { let _ = set(&app, "Visible", Variant::Bool(true)); } register_controlled(&app, progid); Ok(app) } // 已有实例也登记，避免 quit 遗漏
        Err(_) => { // 无运行实例
            let app = create_app(progid)?; // 新建
            if visible { let _ = set(&app, "Visible", Variant::Bool(true)); } // 可见
            let _ = set(&app, "DisplayAlerts", Variant::I4(0)); // 静默（不支持的族忽略）
            register_controlled(&app, progid); // 登记受控
            Ok(app) // 返回
        }
    }
}
pub fn register_controlled(app: &ComObj, label: &'static str) { // 登记受控实例
    CONTROLLED.with(|items| { let mut items = items.borrow_mut(); if items.len() < 16 && !items.iter().any(|item| std::ptr::eq(item.raw(), app.raw())) { items.push(app.clone()); } }); // 保存 COM 引用（去重）
    let list = CONTROLLED_LABELS.get_or_init(|| Mutex::new(Vec::new())); // 标记表
    if let Ok(mut labels) = list.lock() { if !labels.contains(&label) { labels.push(label); } } // 去重
}
pub fn take_controlled() -> Vec<ComObj> { CONTROLLED.with(|items| items.borrow_mut().drain(..).collect()) } // 取出并清空
pub fn controlled_labels() -> Vec<&'static str> { CONTROLLED_LABELS.get_or_init(|| Mutex::new(Vec::new())).lock().map(|labels| labels.clone()).unwrap_or_default() } // 已登记族
pub fn has_controlled() -> bool { CONTROLLED.with(|items| !items.borrow().is_empty()) } // 是否有受控实例

pub fn application_name(app: &ComObj) -> String { get(app, "Name").map(|value| value.as_string()).unwrap_or_default() } // 应用名
pub fn missing_app(progid: &str) -> ToolError { ToolError::new("APP_NOT_INSTALLED", format!("本机未安装或未注册 {progid}")) } // 未安装错误