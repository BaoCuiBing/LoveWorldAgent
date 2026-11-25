use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::dispatch::{call, get, set, ComObj, ComResult, Variant}; // COM 工具
use crate::ppt::locator::{self, add_textbox, arg, f64_arg, i32_arg, set_text, shape_by, str_arg}; // 上下文与辅助
use crate::word::report::{err_json, ok_json, ToolError}; // 信封

pub fn run(args: &Map<String, Value>) -> Value { // ppt_table 5 action
    let action = str_arg(args, "action"); // 动作
    let result: ComResult<Value> = match action.as_str() { // 分派
        "insert" => insert(args), // 插入
        "edit_cell" => edit_cell(args), // 编辑单元格
        "add_row" => add_row(args), // 加行
        "delete_row" => delete_row(args), // 删行
        "merge" => merge(args), // 合并
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 ppt_table action：{other}"))), // 未知
    };
    match result { Ok(value) => ok_json(value), Err(err) => err_json(&err) } // 返回
}
fn slide_of(args: &Map<String, Value>) -> ComResult<ComObj> { // 取幻灯片
    let ctx = locator::open(&str_arg(args, "path"))?; // 上下文
    locator::slide(&ctx, i32_arg(args, "slideIndex", 1)) // 幻灯片
}
fn table_shape(slide: &ComObj, args: &Map<String, Value>) -> ComResult<ComObj> { // 取表格形状
    if arg(args, "shapeIndex").is_some() || arg(args, "shapeName").is_some() { return shape_by(slide, args); } // 指定形状
    let shapes = get(slide, "Shapes")?.as_dispatch()?; // 形状集
    let count = get(&shapes, "Count")?.as_i32(); // 数量
    for index in 1..=count { // 首个表格
        if let Ok(shape) = call(&shapes, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()) { if get(&shape, "HasTable").map(|v| v.as_bool()).unwrap_or(false) { return Ok(shape); } } // 命中
    }
    Err(ToolError::new("INVALID_ARGUMENT", "当前幻灯片没有表格")) // 无表格
}
fn insert(args: &Map<String, Value>) -> ComResult<Value> { // 插入表格
    let slide = slide_of(args)?; // 幻灯片
    let rows = i32_arg(args, "rows", 2).clamp(1, 100); // 行
    let columns = i32_arg(args, "columns", 2).clamp(1, 50); // 列
    let shapes = get(&slide, "Shapes")?.as_dispatch()?; // 形状集
    let shape = call(&shapes, "AddTable", &[Variant::I4(rows), Variant::I4(columns), Variant::R4(f64_arg(args, "left", 60.0) as f32), Variant::R4(f64_arg(args, "top", 160.0) as f32), Variant::R4(f64_arg(args, "width", 480.0) as f32), Variant::R4(f64_arg(args, "height", 180.0) as f32)])?.as_dispatch()?; // 添加
    let table = get(&shape, "Table")?.as_dispatch()?; // 表格
    if let Some(data) = arg(args, "data").and_then(|v| v.as_array()) { // 数据
        for (row, values) in data.iter().enumerate() { // 逐行
            if let Some(cells) = values.as_array() { // 行数组
                for (column, value) in cells.iter().enumerate() { if row < rows as usize && column < columns as usize { write_cell(&table, row as i32 + 1, column as i32 + 1, json_text(value))?; } } // 写
            }
        }
    }
    Ok(json!({"inserted": true, "rows": rows, "columns": columns, "shapeName": get(&shape, "Name").map(|v| v.as_string()).unwrap_or_default()})) // 返回
}
fn edit_cell(args: &Map<String, Value>) -> ComResult<Value> { // 编辑单元格
    let slide = slide_of(args)?; // 幻灯片
    let shape = table_shape(&slide, args)?; // 表格形状
    let table = get(&shape, "Table")?.as_dispatch()?; // 表格
    let row = i32_arg(args, "row", 1); // 行
    let column = i32_arg(args, "column", 1); // 列
    write_cell(&table, row, column, &str_arg(args, "text"))?; // 写
    Ok(json!({"edited": true, "row": row, "column": column})) // 返回
}
fn add_row(args: &Map<String, Value>) -> ComResult<Value> { // 加行
    let slide = slide_of(args)?; // 幻灯片
    let shape = table_shape(&slide, args)?; // 表格形状
    let table = get(&shape, "Table")?.as_dispatch()?; // 表格
    let rows = get(&table, "Rows")?.as_dispatch()?; // 行集合
    call(&rows, "Add", &[])?; // 添加
    let count = get(&rows, "Count").map(|v| v.as_i32()).unwrap_or(0); // 新行数
    if let Some(values) = arg(args, "data").and_then(|v| v.as_array()) { for (column, value) in values.iter().enumerate() { write_cell(&table, count, column as i32 + 1, json_text(value))?; } } // 写数据
    Ok(json!({"added": true, "rows": count})) // 返回
}
fn delete_row(args: &Map<String, Value>) -> ComResult<Value> { // 删行
    let slide = slide_of(args)?; // 幻灯片
    let shape = table_shape(&slide, args)?; // 表格形状
    let table = get(&shape, "Table")?.as_dispatch()?; // 表格
    let rows = get(&table, "Rows")?.as_dispatch()?; // 行集合
    let count = get(&rows, "Count")?.as_i32(); // 数量
    let index = i32_arg(args, "rowIndex", count); // 行号
    if index < 1 || index > count { return Err(ToolError::new("INVALID_ARGUMENT", format!("rowIndex={index} 超出 1..={count}"))); } // 越界
    call(&call(&rows, "Item", &[Variant::I4(index)])?.as_dispatch()?, "Delete", &[])?; // 删除
    Ok(json!({"deleted": true, "rowIndex": index})) // 返回
}
fn merge(args: &Map<String, Value>) -> ComResult<Value> { // 合并单元格
    let slide = slide_of(args)?; // 幻灯片
    let shape = table_shape(&slide, args)?; // 表格形状
    let table = get(&shape, "Table")?.as_dispatch()?; // 表格
    let start = call(&table, "Cell", &[Variant::I4(i32_arg(args, "rowStart", 1)), Variant::I4(i32_arg(args, "colStart", 1))])?.as_dispatch()?; // 起点
    let end = call(&table, "Cell", &[Variant::I4(i32_arg(args, "rowEnd", 1)), Variant::I4(i32_arg(args, "colEnd", 1))])?.as_dispatch()?; // 终点
    call(&start, "Merge", &[Variant::Dispatch(end)])?; // 合并
    Ok(json!({"merged": true})) // 返回
}
fn write_cell(table: &ComObj, row: i32, column: i32, text: &str) -> ComResult<()> { // 写单元格
    let cell = call(table, "Cell", &[Variant::I4(row), Variant::I4(column)])?.as_dispatch()?; // 单元格
    let shape = get(&cell, "Shape")?.as_dispatch()?; // 单元格形状
    set_text(&shape, text) // 写文本
}
fn json_text(value: &Value) -> &str { value.as_str().unwrap_or("") } // JSON 值转文本
#[allow(dead_code)] // 保留文本框兜底入口
fn fallback_textbox(slide: &ComObj) -> ComResult<ComObj> { add_textbox(slide, 60.0, 160.0, 480.0, 120.0, "") } // 备用文本框