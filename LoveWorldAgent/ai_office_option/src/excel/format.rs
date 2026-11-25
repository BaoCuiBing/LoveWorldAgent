use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::constants::MSO_TRUE; // COM 布尔真
use crate::com::dispatch::{call, get, set, ComResult, Variant}; // COM 工具
use crate::excel::locator::{self, arg, f64_arg, i32_arg, rgb, str_arg}; // 上下文与辅助
use crate::word::report::{err_json, ok_json, ToolError}; // 信封

pub fn run(args: &Map<String, Value>) -> Value { // excel_format 7 target
    let target = str_arg(args, "target"); // 目标
    let result: ComResult<Value> = match target.as_str() { // 分派
        "font" => font(args), // 字体
        "fill" => fill(args), // 填充
        "border" => border(args), // 边框
        "number_format" => number_format(args), // 数字格式
        "column_width" => column_width(args), // 列宽
        "row_height" => row_height(args), // 行高
        "merge" => merge(args), // 合并
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 excel_format target：{other}"))), // 未知
    };
    match result { Ok(value) => ok_json(value), Err(err) => err_json(&err) } // 返回
}
fn range_of(args: &Map<String, Value>) -> ComResult<(locator::BookCtx, crate::com::dispatch::ComObj)> { // 取区域
    let ctx = locator::open(&str_arg(args, "path"))?; // 工作簿
    let sheet = locator::sheet(&ctx.book, &str_arg(args, "sheet"))?; // 工作表
    let target = locator::range(&sheet, &str_arg(args, "range"))?; // 区域
    Ok((ctx, target)) // 返回
}
fn font(args: &Map<String, Value>) -> ComResult<Value> { // 字体
    let (_ctx, target) = range_of(args)?; // 区域
    let font = get(&target, "Font")?.as_dispatch()?; // Font
    if let Some(value) = arg(args, "name").and_then(|v| v.as_str()) { set(&font, "Name", Variant::Str(value.to_string()))?; } // 字体名
    if let Some(value) = arg(args, "size").and_then(|v| v.as_f64()) { set(&font, "Size", Variant::R8(value))?; } // 字号
    if let Some(value) = arg(args, "bold").and_then(|v| v.as_bool()) { set(&font, "Bold", Variant::I4(if value { MSO_TRUE } else { 0 }))?; } // 粗体
    if let Some(value) = arg(args, "italic").and_then(|v| v.as_bool()) { set(&font, "Italic", Variant::I4(if value { MSO_TRUE } else { 0 }))?; } // 斜体
    if let Some(value) = arg(args, "color").and_then(|v| v.as_str()).filter(|v| !v.is_empty()) { set(&font, "Color", Variant::I4(rgb(value).ok_or_else(|| ToolError::new("INVALID_ARGUMENT", "color 需为 #RRGGBB"))?))?; } // 颜色
    Ok(json!({"applied": true})) // 返回
}
fn fill(args: &Map<String, Value>) -> ComResult<Value> { // 填充
    let (_ctx, target) = range_of(args)?; // 区域
    let color = str_arg(args, "color"); // 颜色
    let value = rgb(&color).unwrap_or(0xFFFFFF); // 默认白
    let interior = get(&target, "Interior")?.as_dispatch()?; // Interior
    set(&interior, "Color", Variant::I4(value))?; // 设置
    Ok(json!({"applied": true, "color": color})) // 返回
}
fn border(args: &Map<String, Value>) -> ComResult<Value> { // 边框
    let (_ctx, target) = range_of(args)?; // 区域
    let borders = get(&target, "Borders")?.as_dispatch()?; // 边框集
    let index = i32_arg(args, "borderIndex", 1); // 边框索引（1 左 2 右 3 上 4 下 5/6 内）
    let line_style = i32_arg(args, "lineStyle", 1); // 线型
    let item = locator::collection_item(&borders, Variant::I4(index))?; // 边框（Item 方法/属性兼容）
    set(&item, "LineStyle", Variant::I4(line_style))?; // 线型
    if let Some(weight) = args.get("weight").and_then(|v| v.as_i64()) { let _ = set(&item, "Weight", Variant::I4(weight as i32)); } // 粗细
    Ok(json!({"applied": true, "borderIndex": index})) // 返回
}
fn number_format(args: &Map<String, Value>) -> ComResult<Value> { // 数字格式
    let (_ctx, target) = range_of(args)?; // 区域
    let format = str_arg(args, "numberFormat"); // 格式
    if format.is_empty() { return Err(ToolError::new("INVALID_ARGUMENT", "number_format 需要 numberFormat")); } // 校验
    set(&target, "NumberFormat", Variant::Str(format.clone()))?; // 设置
    Ok(json!({"applied": true, "numberFormat": format})) // 返回
}
fn column_width(args: &Map<String, Value>) -> ComResult<Value> { // 列宽
    let (_ctx, target) = range_of(args)?; // 区域
    let width = f64_arg(args, "width", 8.43).clamp(0.0, 255.0); // 宽度
    let columns = get(&target, "Columns")?.as_dispatch()?; // 列
    set(&columns, "ColumnWidth", Variant::R8(width))?; // 设置
    Ok(json!({"applied": true, "width": width})) // 返回
}
fn row_height(args: &Map<String, Value>) -> ComResult<Value> { // 行高
    let (_ctx, target) = range_of(args)?; // 区域
    let height = f64_arg(args, "height", 15.0).clamp(0.0, 409.0); // 高度
    let rows = get(&target, "Rows")?.as_dispatch()?; // 行
    set(&rows, "RowHeight", Variant::R8(height))?; // 设置
    Ok(json!({"applied": true, "height": height})) // 返回
}
fn merge(args: &Map<String, Value>) -> ComResult<Value> { // 合并
    let (_ctx, target) = range_of(args)?; // 区域
    call(&target, "Merge", &[])?; // 合并
    Ok(json!({"merged": true})) // 返回
}