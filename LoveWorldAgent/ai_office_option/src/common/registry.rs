use std::collections::HashMap; // 路径索引
use std::sync::{Mutex, OnceLock}; // 跨线程受控表
use crate::com::dispatch::{get as com_get, ComObj}; // COM 对象与读操作
use crate::word::instances::normalize_path; // 路径规范化
#[derive(Clone)] // 原始登记项（裸指针可跨线程）
struct Entry { // 单文档引用
    app: usize, // Application 裸指针
    doc: usize, // Document/Workbook/Presentation 裸指针
    label: &'static str, // 族标记
}
pub struct DocHandle { // 取回的文档引用
    pub app: ComObj, // Application
    pub doc: ComObj, // Document/Workbook/Presentation
}
static DOCS: OnceLock<Mutex<HashMap<String, Entry>>> = OnceLock::new(); // 路径→文档
fn docs() -> &'static Mutex<HashMap<String, Entry>> { DOCS.get_or_init(|| Mutex::new(HashMap::new())) } // 懒初始化
pub fn get(path: &str, label: &'static str) -> Option<DocHandle> { // 按族取有效登记
    let entry = { docs().lock().ok()?.get(&normalize_path(path)).cloned()? }; // 快照
    if entry.label != label { return None; } // 族不符
    let doc = ComObj::from_raw_ptr(entry.doc)?; // 重建文档
    if com_get(&doc, "FullName").is_err() { forget(path); return None; } // 文档已关闭
    let app = ComObj::from_raw_ptr(entry.app)?; // 重建应用
    Some(DocHandle { app, doc }) // 返回
}
pub fn put(path: &str, label: &'static str, app: &ComObj, doc: &ComObj) { // 登记路径
    if let Ok(mut map) = docs().lock() { map.insert(normalize_path(path), Entry { app: app.leaked_ptr(), doc: doc.leaked_ptr(), label }); } // 写入
}
fn release(entry: &Entry) { unsafe { ComObj::release_raw(entry.app); ComObj::release_raw(entry.doc); } } // 释放登记引用
pub fn forget(path: &str) { // 移除路径
    if let Ok(mut map) = docs().lock() { if let Some(entry) = map.remove(&normalize_path(path)) { release(&entry); } } // 删除并释放
}
pub fn forget_label(label: &'static str) { // 清理族
    if let Ok(mut map) = docs().lock() { map.retain(|_, item| if item.label == label { release(item); false } else { true }); } // 过滤并释放
}
