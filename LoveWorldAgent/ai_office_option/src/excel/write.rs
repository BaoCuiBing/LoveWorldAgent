use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::dispatch::{call, get, set, ComResult, Variant}; // COM 工具
use crate::excel::locator::{self, arg, i32_arg, str_arg}; // 上下文与辅助
use crate::word::report::{err_json, ok_json, ToolError}; // 信封

pub fn run(args: &Map<String, Value>) -> Value { // excel_write 4 action
    let action = str_arg(args, "action"); // 动作
    let result: ComResult<Value> = match action.as_str() { // 分派
        "range" => range(args), // 区域
        "cells" => cells(args), // 单元格
        "markdown" => markdown(args), // Markdown 表格
        "formula" => formula(args), // 公式
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 excel_write action：{other}"))), // 未知
    };
    match result { Ok(value) => ok_json(value), Err(err) => err_json(&err) } // 返回
}
fn sheet_of(args: &Map<String, Value>) -> ComResult<crate::com::dispatch::ComObj> { // 取工作表
    let ctx = locator::open(&str_arg(args, "path"))?; // 工作簿
    locator::sheet(&ctx.book, &str_arg(args, "sheet")) // 工作表
}
fn range(args: &Map<String, Value>) -> ComResult<Value> { // 写区域
    let sheet = sheet_of(args)?; // 工作表
    let address = str_arg(args, "range"); // 地址
    if address.is_empty() { return Err(ToolError::new("INVALID_ARGUMENT", "range 必须传 range 地址")); } // 校验
    let data = arg(args, "data").and_then(|v| v.as_array()).ok_or_else(|| ToolError::new("INVALID_ARGUMENT", "data 必须是二维数组"))?; // 数据
    let count = locator::write_matrix(&sheet, &address, data)?; // 写入
    Ok(json!({"written": true, "cells": count, "range": address})) // 返回
}
fn cells(args: &Map<String, Value>) -> ComResult<Value> { // 写单元格
    let sheet = sheet_of(args)?; // 工作表
    let row = i32_arg(args, "row", 1); // 行
    let column = i32_arg(args, "column", 1); // 列
    let value = arg(args, "value").cloned().unwrap_or(Value::Null); // 值
    locator::write_cell(&locator::cell(&sheet, row, column)?, &value)?; // 写入
    Ok(json!({"written": true, "row": row, "column": column})) // 返回
}
fn markdown(args: &Map<String, Value>) -> ComResult<Value> { // Markdown 表格
    let sheet = sheet_of(args)?; // 工作表
    let source = str_arg(args, "markdown"); // 源
    let rows = parse_markdown(&source); // 解析
    if rows.is_empty() { return Err(ToolError::new("INVALID_ARGUMENT", "markdown 未解析出表格行")); } // 空
    let address = if str_arg(args, "range").is_empty() { "A1".to_string() } else { str_arg(args, "range") }; // 起始
    let count = locator::write_matrix(&sheet, &address, &rows)?; // 写入
    Ok(json!({"written": true, "cells": count, "rows": rows.len()})) // 返回
}
fn formula(args: &Map<String, Value>) -> ComResult<Value> { // 写公式
    let sheet = sheet_of(args)?; // 工作表
    let address = str_arg(args, "range"); // 区域
    let target = if !address.is_empty() { locator::range(&sheet, &address)? } else { locator::cell(&sheet, i32_arg(args, "row", 1), i32_arg(args, "column", 1))? }; // 目标
    let formula = str_arg(args, "formula"); // 公式
    if formula.is_empty() { return Err(ToolError::new("INVALID_ARGUMENT", "formula 不能为空")); } // 校验
    set(&target, "Formula", Variant::Str(formula.clone()))?; // 写入
    Ok(json!({"written": true, "formula": formula})) // 返回
}
fn parse_markdown(source: &str) -> Vec<Value> { // 解析 Markdown 表格
    let mut rows = Vec::new(); // 结果
    for line in source.lines() { // 逐行
        let text = line.trim(); // 去空白
        if !text.starts_with('|') || !text.ends_with('|') { continue; } // 非表格行
        let cells: Vec<&str> = text.trim_matches('|').split('|').map(|cell| cell.trim()).collect(); // 拆分
        if cells.iter().all(|cell| cell.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')) { continue; } // 分隔行
        rows.push(Value::Array(cells.into_iter().map(|cell| Value::String(cell.to_string())).collect())); // 收集
    }
    rows // 返回
}