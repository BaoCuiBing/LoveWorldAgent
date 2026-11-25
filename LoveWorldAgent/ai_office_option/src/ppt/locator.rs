use serde_json::{json, Map, Value}; // JSON 构造
use std::path::{Path, PathBuf}; // 路径类型
use crate::com::constants::*; // Office 常量
use crate::com::dispatch::{call, call_named, get, set, ComObj, ComResult, Variant}; // COM 工具
use crate::common::paths::Paths; // 路径白名单
use crate::common::progid::ensure_app; // 应用附着/创建
use crate::word::instances::same_path; // 路径等价
use crate::word::report::ToolError; // 错误类型

pub struct PresCtx { // 演示文稿上下文
    pub app: ComObj, // Application
    pub pres: ComObj, // Presentation
    pub path: PathBuf, // 完整路径
}

pub fn arg<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a Value> { args.get(key) } // 取参数
pub fn str_arg(args: &Map<String, Value>, key: &str) -> String { arg(args, key).and_then(|v| v.as_str()).unwrap_or("").to_string() } // 字符串
pub fn i32_arg(args: &Map<String, Value>, key: &str, d: i32) -> i32 { arg(args, key).and_then(|v| v.as_i64()).unwrap_or(d as i64) as i32 } // 整数
pub fn f64_arg(args: &Map<String, Value>, key: &str, d: f64) -> f64 { arg(args, key).and_then(|v| v.as_f64()).unwrap_or(d) } // 浮点
pub fn bool_arg(args: &Map<String, Value>, key: &str, d: bool) -> bool { arg(args, key).and_then(|v| v.as_bool()).unwrap_or(d) } // 布尔

pub fn presentations(app: &ComObj) -> ComResult<ComObj> { get(app, "Presentations")?.as_dispatch() } // 演示文稿集合

pub fn presentation_of(app: &ComObj, full: &Path) -> ComResult<Option<ComObj>> { // 按路径查已打开演示文稿
    let collection = presentations(app)?; // 集合
    let count = get(&collection, "Count")?.as_i32(); // 数量
    for index in 1..=count { // 逐个
        if let Ok(pres) = call(&collection, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()) { // 演示文稿
            let name = get(&pres, "FullName").map(|v| v.as_string()).unwrap_or_default(); // 完整路径
            if same_path(&name, &full.to_string_lossy()) { let _ = call(&pres, "Activate", &[]); return Ok(Some(pres)); } // 命中
        }
    }
    Ok(None) // 未命中
}

pub fn open(path: &str) -> ComResult<PresCtx> { // 打开演示文稿（已打开则激活）
    let paths = Paths::load(); // 路径策略
    let full = paths.resolve(path)?; // 校验路径
    if !full.is_file() { return Err(ToolError::new("FILE_NOT_FOUND", format!("演示文稿不存在：{}", full.display()))); } // 不存在
    let full_str = full.to_string_lossy().to_string(); // 字符串路径
    if let Some(entry) = crate::common::registry::get(&full_str, POWERPOINT_PROGID) { // 路径已登记
        let _ = call(&entry.doc, "Activate", &[]); // 激活
        return Ok(PresCtx { app: entry.app, pres: entry.doc, path: full }); // 复用
    }
    let app = ensure_app(POWERPOINT_PROGID, true)?; // 附着或创建
    if let Some(pres) = presentation_of(&app, &full)? { crate::common::registry::put(&full_str, POWERPOINT_PROGID, &app, &pres); return Ok(PresCtx { app, pres, path: full }); } // 已打开
    let collection = presentations(&app)?; // 集合
    let pres = call(&collection, "Open", &[Variant::Str(full_str.clone()), Variant::I4(MSO_FALSE), Variant::I4(MSO_FALSE), Variant::I4(MSO_TRUE)])?.as_dispatch()?; // 打开
    let _ = call(&pres, "Activate", &[]); // 激活
    crate::common::registry::put(&full_str, POWERPOINT_PROGID, &app, &pres); // 登记路径
    Ok(PresCtx { app, pres, path: full }) // 返回
}

pub fn create(path: &str) -> ComResult<PresCtx> { // 新建并立即保存
    let paths = Paths::load(); // 路径策略
    let full = paths.resolve(path)?; // 校验路径
    if full.exists() { return Err(ToolError::new("FILE_EXISTS", format!("文件已存在：{}", full.display()))); } // 已存在
    if let Some(parent) = full.parent() { if !parent.is_dir() { return Err(ToolError::new("INVALID_ARGUMENT", format!("父目录不存在：{}", parent.display()))); } } // 父目录
    let app = ensure_app(POWERPOINT_PROGID, true)?; // 附着或创建
    let collection = presentations(&app)?; // 集合
    let pres = call(&collection, "Add", &[])?.as_dispatch()?; // 新建
    let slides = get(&pres, "Slides")?.as_dispatch()?; // 幻灯片集
    if get(&slides, "Count").map(|v| v.as_i32()).unwrap_or(0) == 0 { let _ = call(&slides, "Add", &[Variant::I4(1), Variant::I4(1)]); } // 确保至少一张
    call(&pres, "SaveAs", &[Variant::Str(full.to_string_lossy().to_string()), Variant::I4(PP_SAVE_AS_OPEN_XML_PRESENTATION)])?; // 保存为 pptx
    crate::common::registry::put(&full.to_string_lossy(), POWERPOINT_PROGID, &app, &pres); // 登记路径
    Ok(PresCtx { app, pres, path: full }) // 返回
}

pub fn ensure(path: &str, create_when_missing: bool) -> ComResult<PresCtx> { // 打开或新建
    let paths = Paths::load(); // 策略
    let full = paths.resolve(path)?; // 路径
    if full.is_file() { return open(path); } // 已存在直接打开
    if create_when_missing { return create(path); } // 允许新建
    Err(ToolError::new("FILE_NOT_FOUND", format!("演示文稿不存在：{}", full.display()))) // 不存在
}

pub fn slide(ctx: &PresCtx, index: i32) -> ComResult<ComObj> { // 按序号取幻灯片
    let slides = get(&ctx.pres, "Slides")?.as_dispatch()?; // 集合
    let count = get(&slides, "Count")?.as_i32(); // 数量
    let index = index.clamp(1, count.max(1)); // 限制范围
    if count == 0 { return Err(ToolError::new("INVALID_ARGUMENT", "演示文稿没有幻灯片")); } // 无幻灯片
    call(&slides, "Item", &[Variant::I4(index)])?.as_dispatch() // 返回
}

pub fn shape_at(slide: &ComObj, index: i32) -> ComResult<ComObj> { // 按序号取形状
    let shapes = get(slide, "Shapes")?.as_dispatch()?; // 形状集
    let count = get(&shapes, "Count")?.as_i32(); // 数量
    if index < 1 || index > count { return Err(ToolError::new("INVALID_ARGUMENT", format!("shapeIndex={index} 超出 1..={count}"))); } // 越界
    call(&shapes, "Item", &[Variant::I4(index)])?.as_dispatch() // 返回
}

pub fn shape_by(slide: &ComObj, args: &Map<String, Value>) -> ComResult<ComObj> { // 按序号或名称取形状
    let name = str_arg(args, "shapeName"); // 名称
    if !name.is_empty() { // 按名称
        let shapes = get(slide, "Shapes")?.as_dispatch()?; // 形状集
        return call(&shapes, "Item", &[Variant::Str(name)]).and_then(|v| v.as_dispatch()).map_err(|_| ToolError::new("INVALID_ARGUMENT", "未找到指定 shapeName")); // 返回
    }
    shape_at(slide, i32_arg(args, "shapeIndex", 1)) // 按序号
}

pub fn text_of(shape: &ComObj) -> String { // 形状文本
    get(shape, "TextFrame").ok().and_then(|v| v.as_dispatch().ok()).and_then(|frame| get(&frame, "TextRange").ok()).and_then(|range| range.as_dispatch().ok()).and_then(|range| get(&range, "Text").ok()).map(|v| v.as_string()).unwrap_or_default() // 读 Text
}

pub fn set_text(shape: &ComObj, text: &str) -> ComResult<()> { // 写形状文本
    let frame = get(shape, "TextFrame")?.as_dispatch()?; // TextFrame
    let range = get(&frame, "TextRange")?.as_dispatch()?; // TextRange
    set(&range, "Text", Variant::Str(text.to_string())) // 写文本
}

pub fn add_textbox(slide: &ComObj, left: f64, top: f64, width: f64, height: f64, text: &str) -> ComResult<ComObj> { // 添加文本框
    let shapes = get(slide, "Shapes")?.as_dispatch()?; // 形状集
    let shape = call(&shapes, "AddTextbox", &[Variant::I4(1), Variant::R4(left as f32), Variant::R4(top as f32), Variant::R4(width as f32), Variant::R4(height as f32)])?.as_dispatch()?; // 水平文本框
    set_text(&shape, text)?; // 写文本
    Ok(shape) // 返回
}

pub fn rgb(hex: &str) -> Option<i32> { // #RRGGBB → BGR
    let text = hex.trim().trim_start_matches('#'); // 去 #
    if text.len() != 6 { return None; } // 长度校验
    let r = i32::from_str_radix(&text[0..2], 16).ok()?; // 红
    let g = i32::from_str_radix(&text[2..4], 16).ok()?; // 绿
    let b = i32::from_str_radix(&text[4..6], 16).ok()?; // 蓝
    Some(r + g * 256 + b * 65536) // BGR
}

pub fn info_json(ctx: &PresCtx) -> ComResult<Value> { // 演示文稿信息
    let slides = get(&ctx.pres, "Slides")?.as_dispatch()?; // 幻灯片集
    let count = get(&slides, "Count")?.as_i32(); // 数量
    let name = get(&ctx.pres, "Name").map(|v| v.as_string()).unwrap_or_default(); // 文件名
    let saved = get(&ctx.pres, "Saved").map(|v| v.as_bool()).unwrap_or(true); // 已保存
    let read_only = get(&ctx.pres, "ReadOnly").map(|v| v.as_bool()).unwrap_or(false); // 只读
    Ok(json!({"path": ctx.path.to_string_lossy(), "name": name, "slideCount": count, "saved": saved, "readOnly": read_only})) // 返回
}

pub fn save(ctx: &PresCtx) -> ComResult<Value> { // 保存并备份
    let backup = crate::word::backup::backup_before_save(&ctx.path)?; // 备份
    call(&ctx.pres, "Save", &[])?; // 保存
    let bytes = std::fs::metadata(&ctx.path).map(|m| m.len() as i64).unwrap_or(0); // 字节数
    Ok(json!({"path": ctx.path.to_string_lossy(), "bytes": bytes, "backupPath": backup.map(|p| p.to_string_lossy().to_string())})) // 返回
}

pub fn close(ctx: &PresCtx, save_changes: bool) -> ComResult<()> { // 关闭演示文稿
    if save_changes { call(&ctx.pres, "Save", &[])?; } else { set(&ctx.pres, "Saved", Variant::Bool(true))?; } // 保存或标记已保存（避免 PowerPoint 弹保存提示）
    call(&ctx.pres, "Close", &[]).map(|_| ()) // 无参关闭，规避 Close(SaveChanges) 的参数兼容问题
}

pub fn export_pdf(ctx: &PresCtx, target: &str) -> ComResult<Value> { // 导出 PDF
    let full = Paths::load().resolve(target)?; // 目标路径
    let path = Variant::Str(full.to_string_lossy().to_string()); // 路径变体
    call(&ctx.pres, "SaveCopyAs", &[path.clone(), Variant::I4(PP_SAVE_AS_PDF)]) // 优先 SaveCopyAs（不改当前格式）
        .or_else(|_| call(&ctx.pres, "ExportAsFixedFormat", &[path, Variant::I4(PP_FIXED_FORMAT_PDF)]))?; // 回退导出
    Ok(json!({"pdfPath": full.to_string_lossy()})) // 返回
}