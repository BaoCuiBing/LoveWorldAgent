use crate::com::app::AppHandle; // 应用句柄
use crate::com::constants::*; // Word 常量
use crate::com::dispatch::{call, get, set, ComObj, ComResult, Variant}; // COM 工具
use crate::word::report::ToolError; // 错误类型
use crate::word::{instances, paths::Paths}; // 实例与路径
use std::path::PathBuf; // 路径
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering}; // DIALOG 枚举状态
use windows::Win32::Foundation::{HWND, LPARAM}; // 窗口基础类型
use windows::core::BOOL; // 回调返回类型
use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetClassNameW, GetWindowThreadProcessId, IsWindowVisible}; // 窗口 API
const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024; // 单文件 50MB 上限
static DIALOG_PID: AtomicU32 = AtomicU32::new(0); // 目标进程
static DIALOG_FOUND: AtomicBool = AtomicBool::new(false); // 是否命中 #32770
pub struct DocCtx { // 文档上下文
    pub app: AppHandle, // 所属应用
    pub doc: ComObj, // Document 对象
    pub path: PathBuf, // 完整路径
}
impl DocCtx { // 文档操作
    pub fn full_name(&self) -> String { // 读 FullName
        get(&self.doc, "FullName").map(|v| v.as_string()).unwrap_or_default() // 委托
    }
    pub fn saved(&self) -> bool { // 读 Saved
        get(&self.doc, "Saved").map(|v| v.as_bool()).unwrap_or(true) // 委托
    }
    pub fn read_only(&self) -> bool { // 是否只读文档
        get(&self.doc, "ReadOnly").map(|v| v.as_bool()).unwrap_or(false) // 委托
    }
    pub fn save(&self, backup_path: Option<&PathBuf>) -> ComResult<i64> { // 保存（备份由调用方完成）
        let _ = backup_path; // 备份路径仅用于审计
        call(&self.doc, "Save", &[])?; // 保存
        let bytes = std::fs::metadata(&self.path).map(|m| m.len() as i64).unwrap_or(0); // 字节数
        Ok(bytes) // 返回
    }
    pub fn close(&self, save_changes: bool) -> ComResult<()> { // 关闭文档
        let flag = if save_changes { WD_SAVE_CHANGES } else { WD_DO_NOT_SAVE_CHANGES }; // 保存标志
        call(&self.doc, "Close", &[Variant::I4(flag)])?; // 关闭
        Ok(()) // 完成
    }
}
pub fn open_document(path: &str, paths: &Paths, force_new_window: bool) -> ComResult<(DocCtx, bool)> { // 打开或附着文档，返回是否已打开
    let _ = force_new_window; // 已打开时直接接管，忽略新窗口参数
    let full = paths.resolve(path)?; // 校验路径
    let full_str = full.to_string_lossy().to_string(); // 字符串路径
    if !full.is_file() { return Err(ToolError::new("FILE_NOT_FOUND", format!("文件不存在：{full_str}"))); } // 文件不存在
    let size = std::fs::metadata(&full).map(|m| m.len()).unwrap_or(0); // 文件大小
    if size > MAX_FILE_BYTES { return Err(ToolError::new("INVALID_ARGUMENT", format!("文件超过 50MB：{size}"))); } // 大小限制
    if let Some(entry) = crate::common::registry::get(&full_str, WORD_PROGID) { // 路径已登记
        call(&entry.doc, "Activate", &[]).ok(); // 激活
        let app = AppHandle { com: entry.app.clone(), controlled: false, progid: WORD_PROGID }; // 附着
        return Ok((DocCtx { app, doc: entry.doc, path: full }, true)); // 复用
    }
    let hits = instances::check_duplicates(&full_str)?; // 查重
    if let Some(first) = hits.first() { // 已打开：附首个，关闭并回收其余
        for dup in hits.iter().skip(1) { // 关闭重复
            call(&dup.doc, "Close", &[Variant::I4(WD_DO_NOT_SAVE_CHANGES)]).ok(); // 关闭
            let count = get(&dup.app, "Documents").and_then(|v| v.as_dispatch()).and_then(|docs| get(&docs, "Count")).map(|v| v.as_i32()).unwrap_or(0); // 文档数
            if count == 0 { call(&dup.app, "Quit", &[]).ok(); } // 回收空实例
        }
        let app = AppHandle { com: first.app.clone(), controlled: false, progid: WORD_PROGID }; // 附着
        crate::com::app::register_controlled_app(app.com.clone()); // 登记受控，避免 quit 遗漏
        crate::common::registry::put(&full_str, WORD_PROGID, &app.com, &first.doc); // 登记路径
        call(&first.doc, "Activate", &[]).ok(); // 激活
        return Ok((DocCtx { app, doc: first.doc.clone(), path: full }, true)); // 返回
    }
    let app = AppHandle::create_new()?; // 未打开：新建可见实例
    crate::com::app::register_controlled_app(app.com.clone()); // 登记受控实例
    let docs = get(&app.com, "Documents")?.as_dispatch()?; // Documents 集合
    let doc = call(&docs, "Open", &[Variant::Str(full_str.clone())])?.as_dispatch()?; // 打开
    crate::common::registry::put(&full_str, WORD_PROGID, &app.com, &doc); // 登记路径
    call(&doc, "Activate", &[]).ok(); // 激活
    Ok((DocCtx { app, doc, path: full }, false)) // 返回
}
pub fn create_document(path: &str, paths: &Paths, template: Option<&str>) -> ComResult<DocCtx> { // 新建文档并立即落盘
    let full = paths.resolve(path)?; // 校验路径
    if full.exists() { return Err(ToolError::new("FILE_EXISTS", format!("文件已存在：{}", full.display()))); } // 已存在
    let Some(parent) = full.parent() else { return Err(ToolError::new("INVALID_ARGUMENT", "路径缺少父目录")); }; // 父目录
    if !parent.is_dir() { return Err(ToolError::new("INVALID_ARGUMENT", format!("父目录不存在：{}", parent.display()))); } // 父目录不存在
    let app = AppHandle::create_new()?; // 新建实例
    let docs = get(&app.com, "Documents")?.as_dispatch()?; // 集合
    let doc = if let Some(template) = template { // 指定模板
        let tpl = paths.resolve(template)?; // 校验模板
        if !tpl.is_file() { return Err(ToolError::new("FILE_NOT_FOUND", format!("模板不存在：{}", tpl.display()))); } // 不存在
        call(&docs, "Add", &[Variant::Str(tpl.to_string_lossy().to_string())])?.as_dispatch()? // 基于模板
    } else { call(&docs, "Add", &[])?.as_dispatch()? }; // 空白文档
    call(&doc, "SaveAs2", &[Variant::Str(full.to_string_lossy().to_string()), Variant::I4(WD_FORMAT_DOCUMENT_DEFAULT)])?; // 立即保存
    if let Some(hwnd) = app.active_window_hwnd() { crate::com::app::register_controlled(hwnd); } // 登记受控实例
    crate::com::app::register_controlled_app(app.com.clone()); // 登记 Application
    crate::common::registry::put(&full.to_string_lossy(), WORD_PROGID, &app.com, &doc); // 登记路径
    Ok(DocCtx { app, doc, path: full }) // 返回
}
pub fn active_document(path: &str, paths: &Paths) -> ComResult<DocCtx> { // 按路径取活动文档（先查 ROT，再附着 ActiveDocument）
    let full = paths.resolve(path)?; // 校验
    let full_str = full.to_string_lossy().to_string(); // 字符串
    if !full.is_file() { return Err(ToolError::new("FILE_NOT_FOUND", format!("文件不存在：{full_str}"))); } // 文件不存在
    if let Some(entry) = crate::common::registry::get(&full_str, WORD_PROGID) { // 路径已登记
        let app = AppHandle { com: entry.app.clone(), controlled: false, progid: WORD_PROGID }; // 附着
        return Ok(DocCtx { app, doc: entry.doc, path: full }); // 复用
    }
    let hits = instances::find_open_documents(&full_str)?; // 查 ROT
    if let Some(hit) = hits.first() { // 命中
        let app = AppHandle { com: hit.app.clone(), controlled: false, progid: WORD_PROGID }; // 附着
        crate::com::app::register_controlled_app(app.com.clone()); // 登记受控实例
        crate::common::registry::put(&full_str, WORD_PROGID, &app.com, &hit.doc); // 登记路径
        return Ok(DocCtx { app, doc: hit.doc.clone(), path: full }); // 返回
    }
    let app = match AppHandle::attach_existing() { Ok(app) => app, Err(_) => return Err(ToolError::new("DOC_CHANGED", format!("目标文档未打开：{full_str}")).recoverable("请先打开或激活目标文档")) }; // 无可附着实例
    let active = match get(&app.com, "ActiveDocument").and_then(|v| v.as_dispatch()) { Ok(doc) => doc, Err(_) => return Err(ToolError::new("DOC_CHANGED", format!("没有活动文档：{full_str}")).recoverable("请先打开或激活目标文档")) }; // 活动文档
    let active_full = get(&active, "FullName").map(|v| v.as_string()).unwrap_or_default(); // 路径
    if !instances::same_path(&active_full, &full_str) { // 不一致
        return Err(ToolError::new("DOC_CHANGED", format!("活动文档不是目标：{active_full}").to_string()).recoverable("请先打开或激活目标文档")); // 文档切换
    }
    crate::com::app::register_controlled_app(app.com.clone()); // 登记受控实例
    crate::common::registry::put(&full_str, WORD_PROGID, &app.com, &active); // 登记路径
    Ok(DocCtx { app, doc: active, path: full }) // 返回
}
pub fn builtin_property(doc: &ComObj, name: &str) -> ComResult<ComObj> { // 按默认成员枚举并匹配英文/本地化 Name（DocumentProperties.Item 需 DISPID_VALUE）
    let props = get(doc, "BuiltInDocumentProperties")?.as_dispatch()?; // 属性集合
    let count = get(&props, "Count")?.as_i32(); // 属性数量
    let aliases = property_aliases(name); // 本地化别名
    for index in 1..=count { // 逐项
        if let Some(prop) = property_at(&props, index) { // 默认成员取属性项
            let prop_name = get(&prop, "Name").map(|value| value.as_string()).unwrap_or_default(); // 名称
            let local = get(&prop, "NameLocal").map(|value| value.as_string()).unwrap_or_default(); // 本地名（部分版本无此成员）
            if prop_name.eq_ignore_ascii_case(name) || aliases.iter().any(|alias| prop_name.eq_ignore_ascii_case(alias) || local.eq_ignore_ascii_case(alias)) { return Ok(prop); } // 命中
        }
    }
    if let Some(index) = property_index(name) { if let Some(prop) = property_at(&props, index) { return Ok(prop); } } // 内置索引兜底
    Err(ToolError::new("WORD_ERROR", format!("未找到文档属性 {name}"))) // 缺失
}
fn property_at(props: &ComObj, index: i32) -> Option<ComObj> { // 属性项（默认成员优先，Item 回退）
    let args = [Variant::I4(index)]; // 参数
    crate::com::dispatch::get_args(props, "", &args).ok().and_then(|value| value.as_dispatch().ok()) // 默认成员 PROPERTYGET
        .or_else(|| crate::com::dispatch::call(props, "", &args).ok().and_then(|value| value.as_dispatch().ok())) // 默认成员 METHOD
        .or_else(|| crate::com::dispatch::get_args(props, "Item", &args).ok().and_then(|value| value.as_dispatch().ok())) // Item 属性
        .or_else(|| crate::com::dispatch::call(props, "Item", &args).ok().and_then(|value| value.as_dispatch().ok())) // Item 方法
}
fn property_index(name: &str) -> Option<i32> { // WdBuiltInProperty 索引
    match name.trim().to_lowercase().as_str() { "title" => Some(1), "subject" => Some(2), "author" => Some(3), "keywords" => Some(4), "comments" => Some(5), "category" => Some(19), _ => None } // 常用内置属性
}
fn property_aliases(name: &str) -> Vec<&'static str> { // 本地化名称别名
    match name.trim().to_lowercase().as_str() {
        "title" => vec!["标题"],
        "subject" => vec!["主题"],
        "author" => vec!["作者"],
        "keywords" => vec!["关键词", "关键字"],
        "comments" => vec!["备注", "注释"],
        "category" => vec!["类别", "分类"],
        _ => vec![""],
    }
}
pub fn instance_has_dialog(hwnd: i32) -> bool { // 枚举同进程可见 #32770
    if hwnd == 0 { return false; } // 无窗口
    let mut pid = 0u32; // 进程号
    unsafe { GetWindowThreadProcessId(HWND(hwnd as isize as *mut std::ffi::c_void), Some(&mut pid)); } // 查询
    if pid == 0 { return false; } // 无进程
    DIALOG_PID.store(pid, Ordering::SeqCst); // 目标
    DIALOG_FOUND.store(false, Ordering::SeqCst); // 重置
    let _ = unsafe { EnumWindows(Some(enum_dialog), LPARAM(0)) }; // 枚举顶层窗口
    DIALOG_FOUND.load(Ordering::SeqCst) // 结果
}
unsafe extern "system" fn enum_dialog(hwnd: HWND, _: LPARAM) -> BOOL { // 顶层窗口回调
    if DIALOG_FOUND.load(Ordering::SeqCst) { return BOOL(0); } // 已命中
    let mut pid = 0u32; // 进程号
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)); } // 查询
    if pid != DIALOG_PID.load(Ordering::SeqCst) { return BOOL(1); } // 非目标进程
    let mut class = [0u16; 64]; // 类名缓冲
    let len = unsafe { GetClassNameW(hwnd, &mut class) }; // 取类名
    if len <= 0 { return BOOL(1); } // 失败继续
    if String::from_utf16_lossy(&class[..len as usize]) == "#32770" && unsafe { IsWindowVisible(hwnd) }.as_bool() { // 对话框
        DIALOG_FOUND.store(true, Ordering::SeqCst); // 命中
        return BOOL(0); // 停止
    }
    BOOL(1) // 继续
}
pub fn paragraph_index_at(doc: &ComObj, pos: i32) -> i32 { // 光标/位置所在段落（1 起）
    if pos <= 0 { return 1; } // 文首
    if let Ok(range) = crate::com::dispatch::range(doc, 0, pos) { // 光标前范围
        if let Ok(paras) = get(&range, "Paragraphs").and_then(|v| v.as_dispatch()) { // 段落集
            return get(&paras, "Count").map(|v| v.as_i32() + 1).unwrap_or(1); // 段落数+1
        }
    }
    1 // 兜底
}
pub fn main_body_end_range(doc: &ComObj) -> ComResult<ComObj> { // 主正文末尾折叠 Range（避开表格/页眉）
    let content = get(doc, "Content")?.as_dispatch()?; // 正文
    let end = get(&content, "End")?.as_i32(); // 文末位置
    crate::com::dispatch::range(doc, end, end) // 折叠 Range
}
pub fn is_dialog_blocked(app: &AppHandle) -> bool { // DIALOG 检测：可见 #32770
    instance_has_dialog(app.active_window_hwnd().unwrap_or(0)) // 对话框窗口（Ready=false 常见于正常编辑，不据此拦截）
}
pub fn ensure_writable(app: &AppHandle) -> ComResult<()> { // 写操作前置校验
    if is_dialog_blocked(app) { Err(ToolError::new("DIALOG_BLOCKED", "Word 对话框阻塞，写操作已拒绝").recoverable("关闭 Word 对话框后重试")) } else { Ok(()) } // 拒绝或放行
}