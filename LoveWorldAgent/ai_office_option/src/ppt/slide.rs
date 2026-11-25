use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::dispatch::{call, get, set, ComResult, Variant}; // COM 工具
use crate::ppt::locator::{self, i32_arg, str_arg}; // 上下文与辅助
use crate::word::report::{err_json, ok_json, ToolError}; // 信封

pub fn run(args: &Map<String, Value>) -> Value { // ppt_slide 6 action
    let action = str_arg(args, "action"); // 动作
    let result: ComResult<Value> = match action.as_str() { // 分派
        "add" => add(args), // 新增
        "delete" => delete(args), // 删除
        "duplicate" => duplicate(args), // 复制
        "move" => move_slide(args), // 移动
        "select" => select(args), // 选中
        "layout" => layout(args), // 版式
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 ppt_slide action：{other}"))), // 未知
    };
    match result { Ok(value) => ok_json(value), Err(err) => err_json(&err) } // 返回
}
fn slide(args: &Map<String, Value>) -> ComResult<(locator::PresCtx, crate::com::dispatch::ComObj, i32)> { // 取幻灯片与序号
    let ctx = locator::open(&str_arg(args, "path"))?; // 上下文
    let index = i32_arg(args, "slideIndex", 1).max(1); // 序号
    let value = locator::slide(&ctx, index)?; // 幻灯片
    Ok((ctx, value, index)) // 返回
}
fn add(args: &Map<String, Value>) -> ComResult<Value> { // 新增幻灯片
    let ctx = locator::open(&str_arg(args, "path"))?; // 上下文
    let collection = get(&ctx.pres, "Slides")?.as_dispatch()?; // 集合
    let count = get(&collection, "Count")?.as_i32(); // 当前数量
    let layout = i32_arg(args, "layout", 1); // 版式
    let slide = call(&collection, "Add", &[Variant::I4(count + 1), Variant::I4(layout)])?.as_dispatch()?; // 添加
    let index = get(&slide, "SlideIndex").map(|v| v.as_i32()).unwrap_or(count + 1); // 新序号
    Ok(json!({"added": true, "slideIndex": index, "slideCount": count + 1})) // 返回
}
fn delete(args: &Map<String, Value>) -> ComResult<Value> { // 删除幻灯片
    let (_ctx, value, index) = slide(args)?; // 幻灯片
    call(&value, "Delete", &[])?; // 删除
    Ok(json!({"deleted": true, "slideIndex": index})) // 返回
}
fn duplicate(args: &Map<String, Value>) -> ComResult<Value> { // 复制幻灯片
    let (_ctx, value, index) = slide(args)?; // 幻灯片
    let result = call(&value, "Duplicate", &[])?.as_dispatch()?; // 返回 SlideRange
    let new_index = get(&result, "SlideIndex").map(|v| v.as_i32()).unwrap_or(index + 1); // 新序号
    Ok(json!({"duplicated": true, "sourceIndex": index, "slideIndex": new_index})) // 返回
}
fn move_slide(args: &Map<String, Value>) -> ComResult<Value> { // 移动幻灯片
    let (_ctx, value, index) = slide(args)?; // 幻灯片
    let to = i32_arg(args, "toIndex", index).max(1); // 目标
    call(&value, "MoveTo", &[Variant::I4(to)])?; // 移动
    Ok(json!({"moved": true, "from": index, "to": to})) // 返回
}
fn select(args: &Map<String, Value>) -> ComResult<Value> { // 选中幻灯片
    let (_ctx, value, index) = slide(args)?; // 幻灯片
    call(&value, "Select", &[])?; // 选中
    Ok(json!({"selected": true, "slideIndex": index})) // 返回
}
fn layout(args: &Map<String, Value>) -> ComResult<Value> { // 应用版式
    let (ctx, value, index) = slide(args)?; // 幻灯片
    let layout = i32_arg(args, "layout", 1); // 版式序号
    let master = get(&ctx.pres, "SlideMaster")?.as_dispatch()?; // 母版
    let layouts = get(&master, "CustomLayouts")?.as_dispatch()?; // 版式集
    let count = get(&layouts, "Count").map(|v| v.as_i32()).unwrap_or(0); // 数量
    if layout < 1 || layout > count { return Err(ToolError::new("INVALID_ARGUMENT", format!("layout={layout} 超出 1..={count}"))); } // 越界
    let target = call(&layouts, "Item", &[Variant::I4(layout)])?.as_dispatch()?; // 版式
    set(&value, "CustomLayout", Variant::Dispatch(target))?; // 应用
    Ok(json!({"applied": true, "slideIndex": index, "layout": layout})) // 返回
}