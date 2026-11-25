use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::constants::MSO_TRUE; // COM 布尔真
use crate::com::dispatch::{call, get, set, ComObj, ComResult, Variant}; // COM 工具
use crate::common::paths::Paths; // 路径策略
use crate::ppt::locator::{self, arg, i32_arg, rgb, shape_by, str_arg}; // 上下文与辅助
use crate::word::report::{err_json, ok_json, ToolError}; // 信封

pub fn run(args: &Map<String, Value>) -> Value { // ppt_format 5 target
    let target = str_arg(args, "target"); // 目标
    let result: ComResult<Value> = match target.as_str() { // 分派
        "font" => font(args), // 字体
        "paragraph" => paragraph(args), // 段落
        "background" => background(args), // 背景
        "theme" => theme(args), // 主题
        "alignment" => paragraph(args), // 对齐（与段落共用）
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 ppt_format target：{other}"))), // 未知
    };
    match result { Ok(value) => ok_json(value), Err(err) => err_json(&err) } // 返回
}
fn slide_of(args: &Map<String, Value>) -> ComResult<(locator::PresCtx, ComObj, i32)> { // 取幻灯片
    let ctx = locator::open(&str_arg(args, "path"))?; // 上下文
    let index = i32_arg(args, "slideIndex", 1).max(1); // 序号
    let slide = locator::slide(&ctx, index)?; // 幻灯片
    Ok((ctx, slide, index)) // 返回
}
fn text_range(slide: &ComObj, args: &Map<String, Value>) -> ComResult<ComObj> { // 目标文本范围
    let shape = shape_by(slide, args)?; // 形状
    let frame = get(&shape, "TextFrame")?.as_dispatch()?; // TextFrame
    get(&frame, "TextRange")?.as_dispatch() // TextRange
}
fn font(args: &Map<String, Value>) -> ComResult<Value> { // 字体
    let (_ctx, slide, index) = slide_of(args)?; // 幻灯片
    let target = text_range(&slide, args)?; // 文本范围
    let font = get(&target, "Font")?.as_dispatch()?; // Font
    if let Some(value) = arg(args, "name").and_then(|v| v.as_str()) { set(&font, "Name", Variant::Str(value.to_string()))?; } // 字体名
    if let Some(value) = arg(args, "size").and_then(|v| v.as_f64()) { set(&font, "Size", Variant::R4(value as f32))?; } // 字号
    if let Some(value) = arg(args, "bold").and_then(|v| v.as_bool()) { set(&font, "Bold", Variant::I4(if value { MSO_TRUE } else { 0 }))?; } // 粗体
    if let Some(value) = arg(args, "italic").and_then(|v| v.as_bool()) { set(&font, "Italic", Variant::I4(if value { MSO_TRUE } else { 0 }))?; } // 斜体
    if let Some(value) = arg(args, "underline").and_then(|v| v.as_bool()) { set(&font, "Underline", Variant::I4(if value { MSO_TRUE } else { 0 }))?; } // 下划线
    if let Some(value) = arg(args, "color").and_then(|v| v.as_str()).filter(|v| !v.is_empty()) { let bgr = rgb(value).ok_or_else(|| ToolError::new("INVALID_ARGUMENT", "color 需为 #RRGGBB"))?; set(&get(&font, "Color")?.as_dispatch()?, "RGB", Variant::I4(bgr))?; } // 颜色
    Ok(json!({"applied": true, "slideIndex": index})) // 返回
}
fn paragraph(args: &Map<String, Value>) -> ComResult<Value> { // 段落/对齐
    let (_ctx, slide, index) = slide_of(args)?; // 幻灯片
    let target = text_range(&slide, args)?; // 文本范围
    let format = get(&target, "ParagraphFormat")?.as_dispatch()?; // 段落格式
    if let Some(value) = arg(args, "alignment").and_then(|v| v.as_str()) { // 对齐
        let code = match value { "left" => 1, "center" => 2, "right" => 3, "justify" => 4, _ => 1 }; // ppAlign
        set(&format, "Alignment", Variant::I4(code))?; // 设置
    }
    if let Some(value) = arg(args, "spaceBefore").and_then(|v| v.as_f64()) { set(&format, "SpaceBefore", Variant::R4(value as f32))?; } // 段前
    if let Some(value) = arg(args, "spaceAfter").and_then(|v| v.as_f64()) { set(&format, "SpaceAfter", Variant::R4(value as f32))?; } // 段后
    if let Some(value) = arg(args, "lineSpacing").and_then(|v| v.as_f64()) { set(&format, "LineSpacing", Variant::R4(value as f32))?; } // 行距
    Ok(json!({"applied": true, "slideIndex": index})) // 返回
}
fn background(args: &Map<String, Value>) -> ComResult<Value> { // 背景
    let (_ctx, slide, index) = slide_of(args)?; // 幻灯片
    let color = str_arg(args, "color"); // 颜色
    let value = rgb(&color).unwrap_or(0xFFFFFF); // 默认白色
    set(&slide, "FollowMasterBackground", Variant::I4(0))?; // 不跟随母版
    let fill = get(&slide, "Background")?.as_dispatch()?; // 背景
    let fill = get(&fill, "Fill")?.as_dispatch()?; // 填充
    let _ = call(&fill, "Solid", &[]); // 纯色
    set(&get(&fill, "ForeColor")?.as_dispatch()?, "RGB", Variant::I4(value))?; // 前景色
    Ok(json!({"applied": true, "slideIndex": index})) // 返回
}
fn theme(args: &Map<String, Value>) -> ComResult<Value> { // 主题
    let (ctx, _slide, index) = slide_of(args)?; // 上下文
    let path = str_arg(args, "themePath"); // 主题文件
    if path.is_empty() { return Err(ToolError::new("INVALID_ARGUMENT", "theme 需要 themePath")); } // 校验
    let full = Paths::load().resolve(&path)?; // 校验
    if !full.is_file() { return Err(ToolError::new("FILE_NOT_FOUND", format!("主题文件不存在：{}", full.display()))); } // 不存在
    call(&ctx.pres, "ApplyTemplate", &[Variant::Str(full.to_string_lossy().to_string())])?; // 应用
    Ok(json!({"applied": true, "slideIndex": index, "theme": full.to_string_lossy()})) // 返回
}
#[allow(dead_code)] // 保留排版入口
fn align(slide: &ComObj, args: &Map<String, Value>) -> ComResult<()> { let target = text_range(slide, args)?; let format = get(&target, "ParagraphFormat")?.as_dispatch()?; set(&format, "Alignment", Variant::I4(2))?; Ok(()) } // 居中兜底