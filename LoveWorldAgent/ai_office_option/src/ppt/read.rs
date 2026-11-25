use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::dispatch::{call, get, ComObj, ComResult, Variant}; // COM 工具
use crate::ppt::locator::{self, i32_arg, str_arg, text_of}; // 上下文与辅助
use crate::word::report::{err_json, ok_json, ToolError}; // 信封

pub fn run(args: &Map<String, Value>) -> Value { // ppt_read 6 target
    let target = str_arg(args, "target"); // 目标
    let result: ComResult<Value> = match target.as_str() { // 分派
        "slides" => slides(args), // 幻灯片列表
        "shapes" => shapes(args), // 形状列表
        "text" => text(args), // 文本
        "notes" => notes(args), // 备注
        "tables" => tables(args), // 表格
        "charts" => charts(args), // 图表
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 ppt_read target：{other}"))), // 未知
    };
    match result { Ok(value) => ok_json(value), Err(err) => err_json(&err) } // 返回
}
fn ctx_path(args: &Map<String, Value>) -> ComResult<locator::PresCtx> { locator::open(&str_arg(args, "path")) } // 取演示文稿
fn slides(args: &Map<String, Value>) -> ComResult<Value> { // 幻灯片列表
    let ctx = ctx_path(args)?; // 上下文
    let collection = get(&ctx.pres, "Slides")?.as_dispatch()?; // 集合
    let count = get(&collection, "Count")?.as_i32(); // 数量
    let mut items = Vec::new(); // 结果
    for index in 1..=count { // 逐个
        if let Ok(slide) = call(&collection, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()) { // 幻灯片
            let name = get(&slide, "Name").map(|v| v.as_string()).unwrap_or_default(); // 名称
            let layout = get(&slide, "Layout").map(|v| v.as_i32()).unwrap_or(0); // 版式
            let shape_count = get(&get(&slide, "Shapes")?.as_dispatch()?, "Count").map(|v| v.as_i32()).unwrap_or(0); // 形状数
            items.push(json!({"index": index, "name": name, "layout": layout, "shapeCount": shape_count})); // 收集
        }
    }
    Ok(json!({"slides": items})) // 返回
}
fn shapes(args: &Map<String, Value>) -> ComResult<Value> { // 形状列表
    let ctx = ctx_path(args)?; // 上下文
    let slide = locator::slide(&ctx, i32_arg(args, "slideIndex", 1))?; // 幻灯片
    let collection = get(&slide, "Shapes")?.as_dispatch()?; // 形状集
    let count = get(&collection, "Count")?.as_i32(); // 数量
    let mut items = Vec::new(); // 结果
    for index in 1..=count { // 逐个
        if let Ok(shape) = call(&collection, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()) { // 形状
            items.push(json!({
                "index": index,
                "name": get(&shape, "Name").map(|v| v.as_string()).unwrap_or_default(),
                "type": get(&shape, "Type").map(|v| v.as_i32()).unwrap_or(0),
                "left": get(&shape, "Left").map(|v| v.as_f64()).unwrap_or(0.0),
                "top": get(&shape, "Top").map(|v| v.as_f64()).unwrap_or(0.0),
                "width": get(&shape, "Width").map(|v| v.as_f64()).unwrap_or(0.0),
                "height": get(&shape, "Height").map(|v| v.as_f64()).unwrap_or(0.0),
                "hasText": get(&shape, "HasTextFrame").map(|v| v.as_bool()).unwrap_or(false),
                "hasTable": get(&shape, "HasTable").map(|v| v.as_bool()).unwrap_or(false),
                "hasChart": get(&shape, "HasChart").map(|v| v.as_bool()).unwrap_or(false),
                "text": text_of(&shape),
            })); // 收集
        }
    }
    Ok(json!({"slideIndex": i32_arg(args, "slideIndex", 1), "shapes": items})) // 返回
}
fn text(args: &Map<String, Value>) -> ComResult<Value> { // 幻灯片文本
    let ctx = ctx_path(args)?; // 上下文
    let slide = locator::slide(&ctx, i32_arg(args, "slideIndex", 1))?; // 幻灯片
    let collection = get(&slide, "Shapes")?.as_dispatch()?; // 形状集
    let count = get(&collection, "Count")?.as_i32(); // 数量
    let mut parts = Vec::new(); // 文本片段
    for index in 1..=count { // 逐个
        if let Ok(shape) = call(&collection, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()) { // 形状
            let value = text_of(&shape); // 文本
            if !value.trim().is_empty() { parts.push(value); } // 收集
        }
    }
    Ok(json!({"slideIndex": i32_arg(args, "slideIndex", 1), "text": parts.join("\n")})) // 返回
}
fn notes(args: &Map<String, Value>) -> ComResult<Value> { // 备注
    let ctx = ctx_path(args)?; // 上下文
    let slide = locator::slide(&ctx, i32_arg(args, "slideIndex", 1))?; // 幻灯片
    let page = get(&slide, "NotesPage")?.as_dispatch()?; // 备注页
    let collection = get(&page, "Shapes")?.as_dispatch()?; // 形状集
    let count = get(&collection, "Count")?.as_i32(); // 数量
    let mut value = String::new(); // 备注文本
    for index in 1..=count { // 逐个
        if let Ok(shape) = call(&collection, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()) { // 形状
            let placeholder = get(&shape, "PlaceholderFormat").ok().and_then(|v| v.as_dispatch().ok()).and_then(|format| get(&format, "Type").ok()).map(|v| v.as_i32()).unwrap_or(0); // 占位符类型
            let candidate = text_of(&shape); // 文本
            if placeholder == 2 || (value.is_empty() && !candidate.trim().is_empty() && !candidate.contains("单击此处添加备注")) { value = candidate; } // 备注体
        }
    }
    Ok(json!({"slideIndex": i32_arg(args, "slideIndex", 1), "notes": value})) // 返回
}
fn tables(args: &Map<String, Value>) -> ComResult<Value> { // 表格
    let ctx = ctx_path(args)?; // 上下文
    let slide = locator::slide(&ctx, i32_arg(args, "slideIndex", 1))?; // 幻灯片
    let collection = get(&slide, "Shapes")?.as_dispatch()?; // 形状集
    let count = get(&collection, "Count")?.as_i32(); // 数量
    let mut items = Vec::new(); // 结果
    for index in 1..=count { // 逐个形状
        if let Ok(shape) = call(&collection, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()) { // 形状
            if !get(&shape, "HasTable").map(|v| v.as_bool()).unwrap_or(false) { continue; } // 非表格
            let table = get(&shape, "Table")?.as_dispatch()?; // 表格
            let rows = get(&get(&table, "Rows")?.as_dispatch()?, "Count").map(|v| v.as_i32()).unwrap_or(0); // 行
            let cols = get(&get(&table, "Columns")?.as_dispatch()?, "Count").map(|v| v.as_i32()).unwrap_or(0); // 列
            let mut data = Vec::new(); // 单元格
            for row in 1..=rows { // 逐行
                let mut cells = Vec::new(); // 行数据
                for col in 1..=cols { // 逐列
                    let text = call(&table, "Cell", &[Variant::I4(row), Variant::I4(col)]).ok().and_then(|v| v.as_dispatch().ok()).map(|cell| text_of(&cell)).unwrap_or_default(); // 单元格文本
                    cells.push(Value::String(text)); // 收集
                }
                data.push(Value::Array(cells)); // 行
            }
            items.push(json!({"shapeIndex": index, "rows": rows, "columns": cols, "data": data})); // 收集
        }
    }
    Ok(json!({"slideIndex": i32_arg(args, "slideIndex", 1), "tables": items})) // 返回
}
fn charts(args: &Map<String, Value>) -> ComResult<Value> { // 图表
    let ctx = ctx_path(args)?; // 上下文
    let slide = locator::slide(&ctx, i32_arg(args, "slideIndex", 1))?; // 幻灯片
    let collection = get(&slide, "Shapes")?.as_dispatch()?; // 形状集
    let count = get(&collection, "Count")?.as_i32(); // 数量
    let mut items = Vec::new(); // 结果
    for index in 1..=count { // 逐个形状
        if let Ok(shape) = call(&collection, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()) { // 形状
            if !get(&shape, "HasChart").map(|v| v.as_bool()).unwrap_or(false) { continue; } // 非图表
            let chart = get(&shape, "Chart")?.as_dispatch()?; // 图表
            let chart_type = get(&chart, "ChartType").map(|v| v.as_i32()).unwrap_or(0); // 类型
            let title = get(&chart, "HasTitle").map(|v| v.as_bool()).unwrap_or(false).then(|| get(&chart, "ChartTitle").ok().and_then(|v| v.as_dispatch().ok()).and_then(|obj| get(&obj, "Text").ok()).map(|v| v.as_string()).unwrap_or_default()).unwrap_or_default(); // 标题
            items.push(json!({"shapeIndex": index, "chartType": chart_type, "title": title})); // 收集
        }
    }
    Ok(json!({"slideIndex": i32_arg(args, "slideIndex", 1), "charts": items})) // 返回
}