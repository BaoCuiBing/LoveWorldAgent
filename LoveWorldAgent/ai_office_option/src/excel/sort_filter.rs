use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::dispatch::{call, get, set, ComResult, Variant}; // COM 工具
use crate::excel::locator::{self, i32_arg, str_arg}; // 上下文与辅助
use crate::word::report::{err_json, ok_json, ToolError}; // 信封

pub fn run(args: &Map<String, Value>) -> Value { // excel_sort_filter 4 action
    let action = str_arg(args, "action"); // 动作
    let result: ComResult<Value> = match action.as_str() { // 分派
        "sort" => sort(args), // 排序
        "filter" => filter(args), // 筛选
        "autofilter" => autofilter(args), // 自动筛选
        "clear" => clear(args), // 清除
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 excel_sort_filter action：{other}"))), // 未知
    };
    match result { Ok(value) => ok_json(value), Err(err) => err_json(&err) } // 返回
}
fn context(args: &Map<String, Value>) -> ComResult<crate::com::dispatch::ComObj> { // 取区域
    let ctx = locator::open(&str_arg(args, "path"))?; // 工作簿
    let sheet = locator::sheet(&ctx.book, &str_arg(args, "sheet"))?; // 工作表
    let address = str_arg(args, "range"); // 区域地址
    if address.trim().is_empty() { return Err(ToolError::new("INVALID_ARGUMENT", "range 不能为空")); } // 空地址拒绝
    locator::range(&sheet, &address) // 区域
}
fn sort(args: &Map<String, Value>) -> ComResult<Value> { // 排序
    let target = context(args)?; // 区域
    let key = str_arg(args, "key"); // 排序列（A/B 或列号）
    let column = if key.len() == 1 { key.to_uppercase().as_bytes()[0] as i32 - 64 } else { key.parse::<i32>().unwrap_or(1) }; // 列号
    let cells = get(&target, "Cells")?.as_dispatch()?; // 单元格集
    let key_args = [Variant::I4(1), Variant::I4(column)]; // 键单元格参数
    let key_cell = crate::com::dispatch::get_args(&cells, "Item", &key_args).or_else(|_| crate::com::dispatch::call(&cells, "Item", &key_args))?.as_dispatch()?; // 排序键
    let order = match str_arg(args, "order").to_lowercase().as_str() { "desc" | "descending" => 2, _ => 1 }; // 升/降
    call(&target, "Sort", &[Variant::Dispatch(key_cell), Variant::I4(order)])?; // 位置参数排序（Key1/Order1）
    Ok(json!({"sorted": true, "key": key, "order": if order == 1 { "asc" } else { "desc" }})) // 返回
}
fn filter(args: &Map<String, Value>) -> ComResult<Value> { // 筛选
    let target = context(args)?; // 区域
    let field = i32_arg(args, "field", 1); // 字段
    let criteria = str_arg(args, "criteria"); // 条件
    call(&target, "AutoFilter", &[Variant::I4(field), Variant::Str(criteria.clone())])?; // 设置
    Ok(json!({"filtered": true, "field": field, "criteria": criteria})) // 返回
}
fn autofilter(args: &Map<String, Value>) -> ComResult<Value> { // 打开自动筛选
    let target = context(args)?; // 区域
    call(&target, "AutoFilter", &[])?; // 打开
    Ok(json!({"autofilter": true})) // 返回
}
fn clear(args: &Map<String, Value>) -> ComResult<Value> { // 清除筛选
    let ctx = locator::open(&str_arg(args, "path"))?; // 工作簿
    let sheet = locator::sheet(&ctx.book, &str_arg(args, "sheet"))?; // 工作表
    if let Ok(active) = get(&sheet, "AutoFilter") { let _ = call(&active.as_dispatch()?, "ShowAllData", &[]); } // 显示全部数据
    let _ = set(&sheet, "AutoFilterMode", Variant::Bool(false)); // 关闭筛选
    Ok(json!({"cleared": true})) // 返回
}