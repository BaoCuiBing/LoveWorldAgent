use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::dispatch::{call, get, ComObj, ComResult, Variant}; // COM 工具
use crate::excel::locator::{self, i32_arg, str_arg}; // 上下文与辅助
use crate::word::report::{err_json, ok_json, ToolError}; // 信封

pub fn run(args: &Map<String, Value>) -> Value { // excel_table 5 action
    let action = str_arg(args, "action"); // 动作
    let result: ComResult<Value> = match action.as_str() { // 分派
        "create" => create(args), // 创建
        "delete" => delete(args), // 删除
        "add_row" => add_row(args), // 加行
        "delete_row" => delete_row(args), // 删行
        "filter" => filter(args), // 筛选
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 excel_table action：{other}"))), // 未知
    };
    match result { Ok(value) => ok_json(value), Err(err) => err_json(&err) } // 返回
}
fn context(args: &Map<String, Value>) -> ComResult<(locator::BookCtx, ComObj)> { // 工作簿与表格集
    let ctx = locator::open(&str_arg(args, "path"))?; // 工作簿
    let sheet = locator::sheet(&ctx.book, &str_arg(args, "sheet"))?; // 工作表
    let tables = get(&sheet, "ListObjects")?.as_dispatch()?; // 表格集
    Ok((ctx, tables)) // 返回
}
fn table_item(tables: &ComObj, args: &Map<String, Value>) -> ComResult<ComObj> { // 按名称/序号取表格
    let name = str_arg(args, "name"); // 名称
    if !name.is_empty() { return locator::collection_item(tables, Variant::Str(name)).map_err(|_| ToolError::new("INVALID_ARGUMENT", "未找到表格名称")); } // 名称
    let index = i32_arg(args, "tableIndex", 1); // 序号
    locator::collection_item(tables, Variant::I4(index)).map_err(|_| ToolError::new("INVALID_ARGUMENT", "表格序号不存在")) // 返回
}
fn create(args: &Map<String, Value>) -> ComResult<Value> { // 创建表格
    let (ctx, tables) = context(args)?; // 集合
    let sheet = locator::sheet(&ctx.book, &str_arg(args, "sheet"))?; // 工作表
    let target = locator::range(&sheet, &str_arg(args, "range"))?; // 区域
    let table = call(&tables, "Add", &[Variant::I4(1), Variant::Dispatch(target), Variant::I4(1)])?.as_dispatch()?; // 创建
    let name = str_arg(args, "name"); // 名称
    if !name.is_empty() { crate::com::dispatch::set(&table, "Name", Variant::Str(name.clone()))?; } // 命名
    Ok(json!({"created": true, "name": get(&table, "Name").map(|v| v.as_string()).unwrap_or_default()})) // 返回
}
fn delete(args: &Map<String, Value>) -> ComResult<Value> { // 删除表格
    let (_ctx, tables) = context(args)?; // 集合
    let table = table_item(&tables, args)?; // 表格
    call(&table, "Delete", &[])?; // 删除
    Ok(json!({"deleted": true})) // 返回
}
fn add_row(args: &Map<String, Value>) -> ComResult<Value> { // 加行
    let (_ctx, tables) = context(args)?; // 集合
    let table = table_item(&tables, args)?; // 表格
    let rows = get(&table, "ListRows")?.as_dispatch()?; // 行集
    call(&rows, "Add", &[])?; // 添加
    let count = get(&rows, "Count").map(|v| v.as_i32()).unwrap_or(0); // 数量
    if let Some(values) = args.get("data").and_then(|v| v.as_array()) { let row = locator::collection_item(&rows, Variant::I4(count))?; let range = get(&row, "Range")?.as_dispatch()?; let cells = get(&range, "Cells")?.as_dispatch()?; for (index, value) in values.iter().enumerate() { let cargs = [Variant::I4(1), Variant::I4(index as i32 + 1)]; let cell = crate::com::dispatch::get_args(&cells, "Item", &cargs).or_else(|_| crate::com::dispatch::call(&cells, "Item", &cargs))?.as_dispatch()?; locator::write_cell(&cell, value)?; } } // 写数据
    Ok(json!({"added": true, "rows": count})) // 返回
}
fn delete_row(args: &Map<String, Value>) -> ComResult<Value> { // 删行
    let (_ctx, tables) = context(args)?; // 集合
    let table = table_item(&tables, args)?; // 表格
    let rows = get(&table, "ListRows")?.as_dispatch()?; // 行集
    let count = get(&rows, "Count")?.as_i32(); // 数量
    let index = i32_arg(args, "rowIndex", count); // 行号
    if index < 1 || index > count { return Err(ToolError::new("INVALID_ARGUMENT", format!("rowIndex={index} 超出 1..={count}"))); } // 越界
    call(&locator::collection_item(&rows, Variant::I4(index))?, "Delete", &[])?; // 删除
    Ok(json!({"deleted": true, "rowIndex": index})) // 返回
}
fn filter(args: &Map<String, Value>) -> ComResult<Value> { // 筛选
    let (_ctx, tables) = context(args)?; // 集合
    let table = table_item(&tables, args)?; // 表格
    let target = get(&table, "Range")?.as_dispatch()?; // 区域
    let field = i32_arg(args, "field", 1); // 字段
    let criteria = str_arg(args, "criteria"); // 条件
    if criteria.is_empty() { call(&target, "AutoFilter", &[])?; } else { call(&target, "AutoFilter", &[Variant::I4(field), Variant::Str(criteria.clone())])?; } // 筛选
    Ok(json!({"filtered": true, "field": field, "criteria": criteria})) // 返回
}