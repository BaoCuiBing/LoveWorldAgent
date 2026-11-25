use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::dispatch::{call, set, ComResult, Variant}; // COM 工具
use crate::excel::locator::{self, i32_arg, str_arg}; // 上下文与辅助
use crate::word::report::{err_json, ok_json, ToolError}; // 信封

pub fn run(args: &Map<String, Value>) -> Value { // excel_sheet 5 action
    let action = str_arg(args, "action"); // 动作
    let result: ComResult<Value> = match action.as_str() { // 分派
        "add" => add(args), // 新增
        "delete" => delete(args), // 删除
        "rename" => rename(args), // 重命名
        "copy" => copy(args), // 复制
        "move" => move_sheet(args), // 移动
        "select" => select(args), // 选中
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 excel_sheet action：{other}"))), // 未知
    };
    match result { Ok(value) => ok_json(value), Err(err) => err_json(&err) } // 返回
}
fn sheets(args: &Map<String, Value>) -> ComResult<(locator::BookCtx, crate::com::dispatch::ComObj)> { // 工作簿与工作表集
    let ctx = locator::open(&str_arg(args, "path"))?; // 工作簿
    let collection = crate::com::dispatch::get(&ctx.book, "Worksheets")?.as_dispatch()?; // 集合
    Ok((ctx, collection)) // 返回
}
fn add(args: &Map<String, Value>) -> ComResult<Value> { // 新增工作表
    let (_ctx, collection) = sheets(args)?; // 集合
    let count = crate::com::dispatch::get(&collection, "Count")?.as_i32(); // 数量
    let sheet = call(&collection, "Add", &[])?.as_dispatch()?; // 添加（默认在活动表前）
    let name = str_arg(args, "name"); // 名称
    if !name.is_empty() { set(&sheet, "Name", Variant::Str(name.clone()))?; } // 重命名
    Ok(json!({"added": true, "name": crate::com::dispatch::get(&sheet, "Name").map(|v| v.as_string()).unwrap_or_default(), "sheetCount": count + 1})) // 返回
}
fn delete(args: &Map<String, Value>) -> ComResult<Value> { // 删除工作表
    let (ctx, _collection) = sheets(args)?; // 工作簿
    let sheet = locator::sheet(&ctx.book, &str_arg(args, "sheet"))?; // 目标
    let _ = set(&ctx.app, "DisplayAlerts", Variant::I4(0)); // 静默
    call(&sheet, "Delete", &[])?; // 删除
    let _ = set(&ctx.app, "DisplayAlerts", Variant::I4(1)); // 恢复
    Ok(json!({"deleted": true})) // 返回
}
fn rename(args: &Map<String, Value>) -> ComResult<Value> { // 重命名
    let (ctx, _collection) = sheets(args)?; // 工作簿
    let sheet = locator::sheet(&ctx.book, &str_arg(args, "sheet"))?; // 目标
    let name = str_arg(args, "name"); // 新名称
    if name.is_empty() { return Err(ToolError::new("INVALID_ARGUMENT", "rename 需要 name")); } // 校验
    set(&sheet, "Name", Variant::Str(name.clone()))?; // 写入
    Ok(json!({"renamed": true, "name": name})) // 返回
}
fn copy(args: &Map<String, Value>) -> ComResult<Value> { // 复制工作表
    let (ctx, _collection) = sheets(args)?; // 工作簿
    let source = locator::sheet(&ctx.book, &str_arg(args, "sheet"))?; // 源
    call(&source, "Copy", &[])?; // 复制到新工作簿（默认行为）
    Ok(json!({"copied": true})) // 返回
}
fn move_sheet(args: &Map<String, Value>) -> ComResult<Value> { // 移动工作表
    let (ctx, _collection) = sheets(args)?; // 工作簿
    let source = locator::sheet(&ctx.book, &str_arg(args, "sheet"))?; // 源
    call(&source, "Move", &[])?; // 移动到新工作簿（默认行为）
    Ok(json!({"moved": true})) // 返回
}
fn select(args: &Map<String, Value>) -> ComResult<Value> { // 选中工作表
    let (ctx, _collection) = sheets(args)?; // 工作簿
    let sheet = locator::sheet(&ctx.book, &str_arg(args, "sheet"))?; // 目标
    call(&sheet, "Activate", &[])?; // 激活
    Ok(json!({"selected": true})) // 返回
}