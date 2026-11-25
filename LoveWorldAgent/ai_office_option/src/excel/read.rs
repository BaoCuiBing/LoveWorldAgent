use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::dispatch::{call, get, ComObj, ComResult, Variant}; // COM 工具
use crate::excel::locator::{self, i32_arg, str_arg}; // 上下文与辅助
use crate::word::report::{err_json, ok_json, ToolError}; // 信封

pub fn run(args: &Map<String, Value>) -> Value { // excel_read 5 target
    let target = str_arg(args, "target"); // 目标
    let result: ComResult<Value> = match target.as_str() { // 分派
        "range" => range(args), // 区域
        "sheet" => sheet(args), // 工作表
        "used_range" => used_range(args), // 已用区域
        "cells" => cells(args), // 单元格
        "table" => table(args), // 表格
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 excel_read target：{other}"))), // 未知
    };
    match result { Ok(value) => ok_json(value), Err(err) => err_json(&err) } // 返回
}
fn sheet_of(args: &Map<String, Value>) -> ComResult<(locator::BookCtx, ComObj)> { // 取工作簿与工作表
    let ctx = locator::open(&str_arg(args, "path"))?; // 工作簿
    let sheet = locator::sheet(&ctx.book, &str_arg(args, "sheet"))?; // 工作表
    Ok((ctx, sheet)) // 返回
}
fn address(args: &Map<String, Value>) -> String { // 区域地址（range/address 二选一）
    let value = str_arg(args, "range"); // range
    if !value.is_empty() { value } else { str_arg(args, "address") } // 返回
}
fn range(args: &Map<String, Value>) -> ComResult<Value> { // 读取区域
    let (_ctx, sheet) = sheet_of(args)?; // 工作表
    locator::matrix_of_sheet(&sheet, &address(args)) // 读取
}
fn sheet(args: &Map<String, Value>) -> ComResult<Value> { // 工作表列表
    let ctx = locator::open(&str_arg(args, "path"))?; // 工作簿
    let items = locator::list_sheets(&ctx.book)?; // 列表
    Ok(json!({"sheets": items})) // 返回
}
fn used_range(args: &Map<String, Value>) -> ComResult<Value> { // 已用区域
    let (_ctx, sheet) = sheet_of(args)?; // 工作表
    let used = get(&sheet, "UsedRange")?.as_dispatch()?; // UsedRange
    let mut data = locator::matrix_of_range(&used)?; // 读取
    data["usedRange"] = json!(get(&used, "Address").map(|v| v.as_string()).unwrap_or_default()); // 地址
    Ok(data) // 返回
}
fn cells(args: &Map<String, Value>) -> ComResult<Value> { // 单元格
    let (_ctx, sheet) = sheet_of(args)?; // 工作表
    let row = i32_arg(args, "row", 1); // 行
    let column = i32_arg(args, "column", 1); // 列
    let target = locator::cell(&sheet, row, column)?; // 单元格
    Ok(json!({"row": row, "column": column, "value": locator::read_cell(&target)})) // 返回
}
fn table(args: &Map<String, Value>) -> ComResult<Value> { // 表格
    let (_ctx, sheet) = sheet_of(args)?; // 工作表
    let tables = get(&sheet, "ListObjects")?.as_dispatch()?; // 表格集
    let count = get(&tables, "Count")?.as_i32(); // 数量
    let mut items = Vec::new(); // 结果
    for index in 1..=count { // 逐个
        if let Ok(item) = locator::collection_item(&tables, Variant::I4(index)) { // 表格
            items.push(json!({
                "index": index,
                "name": get(&item, "Name").map(|v| v.as_string()).unwrap_or_default(),
                "range": get(&get(&item, "Range")?.as_dispatch()?, "Address").map(|v| v.as_string()).unwrap_or_default(),
                "rows": get(&get(&item, "ListRows")?.as_dispatch()?, "Count").map(|v| v.as_i32()).unwrap_or(0),
                "columns": get(&get(&item, "ListColumns")?.as_dispatch()?, "Count").map(|v| v.as_i32()).unwrap_or(0),
            })); // 收集
        }
    }
    Ok(json!({"tables": items})) // 返回
}