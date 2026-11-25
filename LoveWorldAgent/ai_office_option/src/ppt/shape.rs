use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::constants::MSO_TRUE; // COM 布尔真
use crate::com::dispatch::{call, get, set, ComObj, ComResult, Variant}; // COM 工具
use crate::common::paths::Paths; // 路径策略
use crate::ppt::locator::{self, add_textbox, arg, f64_arg, i32_arg, rgb, shape_by, str_arg}; // 上下文与辅助
use crate::word::report::{err_json, ok_json, ToolError}; // 信封

pub fn run(args: &Map<String, Value>) -> Value { // ppt_shape 7 action
    let action = str_arg(args, "action"); // 动作
    let result: ComResult<Value> = match action.as_str() { // 分派
        "add_textbox" => add_textbox_action(args), // 文本框
        "add_picture" => add_picture(args), // 图片
        "set_position" => set_position(args), // 位置
        "set_size" => set_size(args), // 尺寸
        "set_fill" => set_fill(args), // 填充
        "set_line" => set_line(args), // 边框
        "delete" => delete(args), // 删除
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 ppt_shape action：{other}"))), // 未知
    };
    match result { Ok(value) => ok_json(value), Err(err) => err_json(&err) } // 返回
}
fn slide_of(args: &Map<String, Value>) -> ComResult<(locator::PresCtx, ComObj, i32)> { // 取幻灯片
    let ctx = locator::open(&str_arg(args, "path"))?; // 上下文
    let index = i32_arg(args, "slideIndex", 1).max(1); // 序号
    let slide = locator::slide(&ctx, index)?; // 幻灯片
    Ok((ctx, slide, index)) // 返回
}
fn add_textbox_action(args: &Map<String, Value>) -> ComResult<Value> { // 添加文本框
    let (_ctx, slide, index) = slide_of(args)?; // 幻灯片
    let shape = add_textbox(&slide, f64_arg(args, "left", 40.0), f64_arg(args, "top", 40.0), f64_arg(args, "width", 320.0), f64_arg(args, "height", 120.0), &str_arg(args, "text"))?; // 添加
    Ok(json!({"added": true, "slideIndex": index, "shapeName": get(&shape, "Name").map(|v| v.as_string()).unwrap_or_default()})) // 返回
}
fn add_picture(args: &Map<String, Value>) -> ComResult<Value> { // 添加图片
    let (_ctx, slide, index) = slide_of(args)?; // 幻灯片
    let image = Paths::load().resolve(&str_arg(args, "imagePath"))?; // 图片路径
    if !image.is_file() { return Err(ToolError::new("FILE_NOT_FOUND", format!("图片不存在：{}", image.display()))); } // 不存在
    let shapes = get(&slide, "Shapes")?.as_dispatch()?; // 形状集
    let shape = call(&shapes, "AddPicture", &[Variant::Str(image.to_string_lossy().to_string()), Variant::I4(MSO_TRUE), Variant::I4(0), Variant::R4(f64_arg(args, "left", 80.0) as f32), Variant::R4(f64_arg(args, "top", 80.0) as f32), Variant::R4(f64_arg(args, "width", 320.0) as f32), Variant::R4(f64_arg(args, "height", 240.0) as f32)])?.as_dispatch()?; // 插入
    Ok(json!({"added": true, "slideIndex": index, "shapeName": get(&shape, "Name").map(|v| v.as_string()).unwrap_or_default()})) // 返回
}
fn set_position(args: &Map<String, Value>) -> ComResult<Value> { // 设置位置
    let (_ctx, slide, index) = slide_of(args)?; // 幻灯片
    let shape = shape_by(&slide, args)?; // 形状
    if let Some(value) = arg(args, "left").and_then(|v| v.as_f64()) { set(&shape, "Left", Variant::R4(value as f32))?; } // 左
    if let Some(value) = arg(args, "top").and_then(|v| v.as_f64()) { set(&shape, "Top", Variant::R4(value as f32))?; } // 上
    Ok(json!({"applied": true, "slideIndex": index})) // 返回
}
fn set_size(args: &Map<String, Value>) -> ComResult<Value> { // 设置尺寸
    let (_ctx, slide, index) = slide_of(args)?; // 幻灯片
    let shape = shape_by(&slide, args)?; // 形状
    if let Some(value) = arg(args, "width").and_then(|v| v.as_f64()) { set(&shape, "Width", Variant::R4(value as f32))?; } // 宽
    if let Some(value) = arg(args, "height").and_then(|v| v.as_f64()) { set(&shape, "Height", Variant::R4(value as f32))?; } // 高
    Ok(json!({"applied": true, "slideIndex": index})) // 返回
}
fn set_fill(args: &Map<String, Value>) -> ComResult<Value> { // 设置填充
    let (_ctx, slide, index) = slide_of(args)?; // 幻灯片
    let shape = shape_by(&slide, args)?; // 形状
    let fill = get(&shape, "Fill")?.as_dispatch()?; // 填充
    let color = str_arg(args, "color"); // 颜色
    if !color.is_empty() { let value = rgb(&color).ok_or_else(|| ToolError::new("INVALID_ARGUMENT", "color 需为 #RRGGBB"))?; set(&get(&fill, "ForeColor")?.as_dispatch()?, "RGB", Variant::I4(value))?; } // 前景色
    let _ = call(&fill, "Solid", &[]); // 纯色填充
    Ok(json!({"applied": true, "slideIndex": index})) // 返回
}
fn set_line(args: &Map<String, Value>) -> ComResult<Value> { // 设置边框
    let (_ctx, slide, index) = slide_of(args)?; // 幻灯片
    let shape = shape_by(&slide, args)?; // 形状
    let line = get(&shape, "Line")?.as_dispatch()?; // 线条
    let visible = args.get("visible").and_then(|v| v.as_bool()).unwrap_or(true); // 可见
    set(&line, "Visible", Variant::I4(if visible { MSO_TRUE } else { 0 }))?; // 显示/隐藏
    if let Some(color) = args.get("color").and_then(|v| v.as_str()).filter(|v| !v.is_empty()) { let value = rgb(color).ok_or_else(|| ToolError::new("INVALID_ARGUMENT", "color 需为 #RRGGBB"))?; set(&get(&line, "ForeColor")?.as_dispatch()?, "RGB", Variant::I4(value))?; } // 颜色
    Ok(json!({"applied": true, "slideIndex": index})) // 返回
}
fn delete(args: &Map<String, Value>) -> ComResult<Value> { // 删除形状
    let (_ctx, slide, index) = slide_of(args)?; // 幻灯片
    let shape = shape_by(&slide, args)?; // 形状
    call(&shape, "Delete", &[])?; // 删除
    Ok(json!({"deleted": true, "slideIndex": index})) // 返回
}