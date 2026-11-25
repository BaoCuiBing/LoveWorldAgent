use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::constants::MSO_TRUE; // COM 布尔真
use crate::com::dispatch::{call, get, set, ComObj, ComResult, Variant}; // COM 工具
use crate::excel::locator::{self, f64_arg, i32_arg, str_arg}; // 上下文与辅助
use crate::word::report::{err_json, ok_json, ToolError}; // 信封

pub fn run(args: &Map<String, Value>) -> Value { // excel_chart 4 action
    let action = str_arg(args, "action"); // 动作
    let result: ComResult<Value> = match action.as_str() { // 分派
        "insert" => insert(args), // 插入
        "update_data" => update_data(args), // 更新数据
        "set_title" => set_title(args), // 标题
        "set_type" => set_type(args), // 类型
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 excel_chart action：{other}"))), // 未知
    };
    match result { Ok(value) => ok_json(value), Err(err) => err_json(&err) } // 返回
}
fn chart_type(name: &str) -> i32 { match name { "bar" => 57, "line" => 4, "pie" => 5, "area" => 1, _ => 51 } } // 类型映射
fn chart_of(args: &Map<String, Value>) -> ComResult<(locator::BookCtx, ComObj)> { // 取指定图表
    let ctx = locator::open(&str_arg(args, "path"))?; // 工作簿
    let sheet = sheet_or_active(&ctx.book, &str_arg(args, "sheet"))?; // 工作表
    let shapes = get(&sheet, "Shapes")?.as_dispatch()?; // 形状集（部分 Excel 不暴露 ChartObjects）
    let count = get(&shapes, "Count").map(|v| v.as_i32()).unwrap_or(0); // 形状数
    let name = str_arg(args, "chartName"); // 名称
    let wanted = i32_arg(args, "chartIndex", 1); // 第几个图表
    let mut seen = 0; // 已遇图表数
    for index in 1..=count { // 逐个形状
        let shape = locator::collection_item(&shapes, Variant::I4(index))?; // 形状
        if !get(&shape, "HasChart").map(|v| v.as_bool()).unwrap_or(false) { continue; } // 跳过非图表
        seen += 1; // 图表计数
        let shape_name = get(&shape, "Name").map(|v| v.as_string()).unwrap_or_default(); // 形状名
        if (!name.is_empty() && shape_name.eq_ignore_ascii_case(&name)) || (name.is_empty() && seen == wanted) { // 命中
            let chart = get(&shape, "Chart")?.as_dispatch()?; // Chart
            return Ok((ctx, chart)); // 返回
        }
    }
    Err(ToolError::new("INVALID_ARGUMENT", "图表不存在或索引越界")) // 未命中
}
fn insert(args: &Map<String, Value>) -> ComResult<Value> { // 插入图表
    let ctx = locator::open(&str_arg(args, "path"))?; // 工作簿
    let sheet = sheet_or_active(&ctx.book, &str_arg(args, "sheet"))?; // 工作表（保证可读 ChartObjects）
    let shapes = get(&sheet, "Shapes")?.as_dispatch()?; // 形状集（避开 ChartObjects 兼容问题）
    let kind = chart_type(&str_arg(args, "type")); // 类型
    let shape = call(&shapes, "AddChart2", &[Variant::I4(-1), Variant::I4(kind), Variant::R8(f64_arg(args, "left", 120.0)), Variant::R8(f64_arg(args, "top", 20.0)), Variant::R8(f64_arg(args, "width", 400.0)), Variant::R8(f64_arg(args, "height", 260.0))])?.as_dispatch()?; // 添加图表
    let chart = get(&shape, "Chart")?.as_dispatch()?; // Chart
    set(&chart, "ChartType", Variant::I4(kind))?; // 类型
    if let Some(range) = args.get("range").and_then(|v| v.as_str()).filter(|v| !v.is_empty()) { let source = locator::range(&sheet, range)?; call(&chart, "SetSourceData", &[Variant::Dispatch(source)])?; } // 数据源
    Ok(json!({"inserted": true, "chartType": kind, "name": get(&shape, "Name").map(|v| v.as_string()).unwrap_or_default()})) // 返回
}
fn update_data(args: &Map<String, Value>) -> ComResult<Value> { // 更新数据源
    let (ctx, chart) = chart_of(args)?; // 图表
    let sheet = sheet_or_active(&ctx.book, &str_arg(args, "sheet"))?; // 工作表（保证可读 ChartObjects）
    let source = locator::range(&sheet, &str_arg(args, "range"))?; // 数据源
    call(&chart, "SetSourceData", &[Variant::Dispatch(source)])?; // 设置
    Ok(json!({"updated": true})) // 返回
}
fn set_title(args: &Map<String, Value>) -> ComResult<Value> { // 标题
    let (_ctx, chart) = chart_of(args)?; // 图表
    let title = str_arg(args, "title"); // 标题
    set(&chart, "HasTitle", Variant::I4(if title.is_empty() { 0 } else { MSO_TRUE }))?; // 显示
    if !title.is_empty() { set(&get(&chart, "ChartTitle")?.as_dispatch()?, "Text", Variant::Str(title.clone()))?; } // 文本
    Ok(json!({"applied": true, "title": title})) // 返回
}
fn set_type(args: &Map<String, Value>) -> ComResult<Value> { // 类型
    let (_ctx, chart) = chart_of(args)?; // 图表
    let kind = chart_type(&str_arg(args, "type")); // 类型
    set(&chart, "ChartType", Variant::I4(kind))?; // 设置
    Ok(json!({"applied": true, "chartType": kind})) // 返回
}fn sheet_or_active(book: &ComObj, name: &str) -> ComResult<ComObj> { // 取工作表并确保支持 ChartObjects
    let sheet = locator::sheet(book, name)?; // 指定工作表
    if get(&sheet, "ChartObjects").is_ok() { return Ok(sheet); } // 正常工作表
    get(book, "ActiveSheet")?.as_dispatch() // 回退活动工作表
}