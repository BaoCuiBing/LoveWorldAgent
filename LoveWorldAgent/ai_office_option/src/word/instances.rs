use windows::Win32::System::Com::{CreateBindCtx, IMoniker, IRunningObjectTable}; // ROT 类型
use crate::com::app::running_object_table; // ROT 获取
use crate::com::dispatch::{get, ComObj, ComResult}; // COM 工具
use crate::word::report::ToolError; // 错误类型
#[derive(Clone)] // 已打开文档引用
pub struct DocRef { // ROT 命中的文档
    pub path: String, // 完整路径
    pub saved: bool, // 是否已保存
    pub doc: ComObj, // Document 对象
    pub app: ComObj, // Application 对象
}
pub fn find_open_documents(path: &str) -> ComResult<Vec<DocRef>> { // ROT 按路径查找打开的文档
    let rot = running_object_table()?; // 取 ROT
    let bind = unsafe { CreateBindCtx(0) }.map_err(|e| crate::com::dispatch::map_hresult(&e, "CreateBindCtx"))?; // 绑定上下文
    let mut out = Vec::new(); // 结果
    for docref in rot_matches(&rot, &bind, path)? { // 遍历命中
        out.push(docref); // 收集
    }
    Ok(out) // 返回
}
fn rot_matches(rot: &IRunningObjectTable, bind: &windows::Win32::System::Com::IBindCtx, path: &str) -> ComResult<Vec<DocRef>> { // 枚举 ROT 中路径匹配的文档
    let target = normalize_path(path); // 规范化目标
    let mut out = Vec::new(); // 结果
    let mut enumm = unsafe { rot.EnumRunning() }.map_err(|e| crate::com::dispatch::map_hresult(&e, "EnumRunning"))?; // 枚举器
    loop { // 逐个 moniker
        let mut array: [Option<IMoniker>; 1] = [None]; // 单元素缓冲
        let mut fetched = 0u32; // 实取数量
        let hr = unsafe { enumm.Next(&mut array, Some(&mut fetched)) }; // 取下一个
        if hr.is_err() || fetched == 0 { break; } // 结束
        let Some(moniker) = array.into_iter().next().flatten() else { break; }; // 取 moniker
        let display = unsafe { moniker.GetDisplayName(Some(bind), None) }.ok(); // 显示名
        let Some(display) = display else { continue; }; // 无显示名跳过
        let name = unsafe { display.to_string() }.unwrap_or_default(); // 转字符串
        if !same_path(&name, &target) { continue; } // 路径不匹配
        let unknown: windows::core::IUnknown = match unsafe { moniker.BindToObject(Some(bind), None) } { Ok(value) => value, Err(_) => continue }; // 绑定对象
        let doc = match ComObj::from_iunknown(unknown) { Ok(value) => value, Err(_) => continue }; // 转 IDispatch
        let full = get(&doc, "FullName").map(|v| v.as_string()).unwrap_or(name); // 完整路径
        let saved = get(&doc, "Saved").map(|v| v.as_bool()).unwrap_or(true); // 保存状态
        let app = match get(&doc, "Application").and_then(|v| v.as_dispatch()) { Ok(value) => value, Err(_) => continue }; // Application
        out.push(DocRef { path: full, saved, doc, app }); // 收集
    }
    Ok(out) // 返回
}
pub fn normalize_path(path: &str) -> String { // 路径规范化（大小写不敏感、去 \\?\ 前缀）
    let trimmed = path.trim(); // 去空白
    let stripped = trimmed.strip_prefix(r"\\?\").unwrap_or(trimmed); // 去设备前缀
    stripped.replace('/', "\\").to_lowercase() // 统一分隔符与大小写
}
pub fn same_path(a: &str, b: &str) -> bool { // 路径等价比较
    normalize_path(a) == normalize_path(b) // 规范化后比较
}
pub fn check_duplicates(path: &str) -> ComResult<Vec<DocRef>> { // 查重：有未保存实例直接拒绝
    let hits = find_open_documents(path)?; // 枚举
    if hits.iter().any(|hit| !hit.saved) { // 存在未保存
        return Err(ToolError::new("DUPLICATE_INSTANCE_UNSAVED", format!("文档 {path} 在另一个 Word 实例中有未保存修改，请先保存或关闭")).recoverable("保存或关闭其它实例后重试")); // 拒绝
    }
    Ok(hits) // 返回已保存命中
}
pub fn all_documents() -> ComResult<Vec<DocRef>> { // 枚举 ROT 中全部 Word 文档（status/quit 用）
    let rot = running_object_table()?; // ROT
    let bind = unsafe { CreateBindCtx(0) }.map_err(|e| crate::com::dispatch::map_hresult(&e, "CreateBindCtx"))?; // 绑定上下文
    let mut out: Vec<DocRef> = Vec::new(); // 结果
    let mut enumm = unsafe { rot.EnumRunning() }.map_err(|e| crate::com::dispatch::map_hresult(&e, "EnumRunning"))?; // 枚举器
    loop { // 遍历
        let mut array: [Option<IMoniker>; 1] = [None]; // 缓冲
        let mut fetched = 0u32; // 实取
        if unsafe { enumm.Next(&mut array, Some(&mut fetched)) }.is_err() || fetched == 0 { break; } // 结束
        let Some(moniker) = array.into_iter().next().flatten() else { break; }; // moniker
        let unknown: windows::core::IUnknown = match unsafe { moniker.BindToObject(Some(&bind), None) } { Ok(value) => value, Err(_) => continue }; // 绑定
        let doc = match ComObj::from_iunknown(unknown) { Ok(value) => value, Err(_) => continue }; // IDispatch
        let full = get(&doc, "FullName").map(|v| v.as_string()).unwrap_or_default(); // 完整路径
        if full.is_empty() || !is_word_file(&full) { continue; } // 非 Word 文档跳过
        let saved = get(&doc, "Saved").map(|v| v.as_bool()).unwrap_or(true); // 保存状态
        let app = match get(&doc, "Application").and_then(|v| v.as_dispatch()) { Ok(value) => value, Err(_) => continue }; // Application
        let hwnd = application_hwnd(&app); // 实例窗口
        if out.iter().any(|hit| same_path(&hit.path, &full) && application_hwnd(&hit.app) == hwnd) { continue; } // 同实例同文档去重
        out.push(DocRef { path: full, saved, doc, app }); // 收集
    }
    Ok(out) // 返回
}
pub fn application_hwnd(app: &ComObj) -> i32 { // 取 Application 活动窗口句柄
    get(app, "ActiveWindow").ok().and_then(|value| value.as_dispatch().ok()).and_then(|window| get(&window, "Hwnd").ok()).map(|value| value.as_i32()).unwrap_or(0) // 读 Hwnd
}
fn is_word_file(path: &str) -> bool { // 按扩展名过滤 Word 文档
    let lower = path.to_lowercase(); // 小写
    [".docx", ".doc", ".rtf", ".txt", ".html", ".htm", ".mht", ".xml", ".odt", ".dotx", ".dotm", ".docm"].iter().any(|ext| lower.ends_with(ext)) // 白名单
}

#[cfg(test)] // 单元测试
mod tests { // 路径等价与去重辅助
    use super::*; // 引入
    #[test] // 大小写与分隔符不敏感
    fn same_path_ignores_case() { assert!(same_path("C:/A/b.docx", "c:/a/B.DOCX")); } // 相等
    #[test] // 不同路径
    fn detects_different_paths() { assert!(!same_path("C:/A/b.docx", "C:/A/c.docx")); } // 不等
    #[test] // 规范化输出小写
    fn normalize_lowercases() { assert_eq!(normalize_path("C:/A/b.docx").to_lowercase(), normalize_path("C:/A/b.docx")); } // 幂等
}
