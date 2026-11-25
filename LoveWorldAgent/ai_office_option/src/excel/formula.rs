use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::dispatch::{call, get, set, ComResult, Variant}; // COM 工具
use crate::excel::locator::{self, i32_arg, str_arg}; // 上下文与辅助
use crate::word::report::{err_json, ok_json, ToolError}; // 信封

pub fn run(args: &Map<String, Value>) -> Value { // excel_formula 3 action
    let action = str_arg(args, "action"); // 动作
    let result: ComResult<Value> = match action.as_str() { // 分派
        "set" => set_formula(args), // 写公式
        "get" => get_formula(args), // 读公式
        "recalculate" => recalculate(args), // 重算
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 excel_formula action：{other}"))), // 未知
    };
    match result { Ok(value) => ok_json(value), Err(err) => err_json(&err) } // 返回
}
fn target(args: &Map<String, Value>) -> ComResult<(locator::BookCtx, crate::com::dispatch::ComObj)> { // 取目标区域
    let ctx = locator::open(&str_arg(args, "path"))?; // 工作簿
    let sheet = locator::sheet(&ctx.book, &str_arg(args, "sheet"))?; // 工作表
    let cell = str_arg(args, "cell"); // 单元格地址优先
    let address = str_arg(args, "range"); // 区域地址
    let value = if !cell.is_empty() { locator::range(&sheet, &cell)? } else if !address.is_empty() { locator::range(&sheet, &address)? } else { locator::cell(&sheet, i32_arg(args, "row", 1), i32_arg(args, "column", 1))? }; // 单元格/区域/行列
    Ok((ctx, value)) // 返回
}
fn set_formula(args: &Map<String, Value>) -> ComResult<Value> { // 写公式
    let (ctx, value) = target(args)?; // 目标
    let cell = str_arg(args, "cell"); // 单元格地址
    let target = if !cell.is_empty() { locator::range(&locator::sheet(&ctx.book, &str_arg(args, "sheet"))?, &cell)? } else { value }; // 单元格优先
    let formula = str_arg(args, "formula"); // 公式
    if formula.is_empty() { return Err(ToolError::new("INVALID_ARGUMENT", "formula 不能为空")); } // 校验
    set(&target, "Formula", Variant::Str(formula.clone()))?; // 写入
    Ok(json!({"set": true, "formula": formula})) // 返回
}
fn get_formula(args: &Map<String, Value>) -> ComResult<Value> { // 读公式
    let (_ctx, target) = target(args)?; // 目标
    let formula = get(&target, "Formula").map(|v| v.as_string()).unwrap_or_default(); // 公式
    let text = get(&target, "Text").map(|v| v.as_string()).unwrap_or_default(); // 显示文本
    let value = locator::read_cell(&target); // 值
    Ok(json!({"formula": formula, "text": text, "value": value})) // 返回
}
fn recalculate(args: &Map<String, Value>) -> ComResult<Value> { // 重算
    let ctx = locator::open(&str_arg(args, "path"))?; // 工作簿
    call(&ctx.app, "Calculate", &[])?; // 重算
    Ok(json!({"recalculated": true})) // 返回
}