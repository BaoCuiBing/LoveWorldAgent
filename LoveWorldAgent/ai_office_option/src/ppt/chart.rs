use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::constants::MSO_TRUE; // COM 布尔真
use crate::com::dispatch::{call, get, set, ComObj, ComResult, Variant}; // COM 工具
use crate::ppt::locator::{self, add_textbox, arg, f64_arg, i32_arg, shape_by, str_arg}; // 上下文与辅助
use crate::word::report::{err_json, ok_json, ToolError}; // 信封

pub fn run(args: &Map<String, Value>) -> Value { // ppt_chart 4 action
    let action = str_arg(args, "action"); // 动作
    let result: ComResult<Value> = match action.as_str() { // 分派
        "insert" => insert(args), // 插入
        "update_data" => update_data(args), // 更新数据
        "set_title" => set_title(args), // 标题
        "set_type" => set_type(args), // 类型
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 ppt_chart action：{other}"))), // 未知
    };
    match result { Ok(value) => ok_json(value), Err(err) => err_json(&err) } // 返回
}
fn chart_type(name: &str) -> i32 { // 图表类型
    match name { "bar" => 57, "line" => 4, "pie" => 5, "area" => 1, _ => 51 } // 默认簇状柱形
}
fn slide_of(args: &Map<String, Value>) -> ComResult<ComObj> { // 取幻灯片
    let ctx = locator::open(&str_arg(args, "path"))?; // 上下文
    locator::slide(&ctx, i32_arg(args, "slideIndex", 1)) // 幻灯片
}
fn chart_shape(slide: &ComObj, args: &Map<String, Value>) -> ComResult<ComObj> { // 取图表形状
    if arg(args, "shapeIndex").is_some() || arg(args, "shapeName").is_some() { return shape_by(slide, args); } // 指定形状
    let shapes = get(slide, "Shapes")?.as_dispatch()?; // 形状集
    let count = get(&shapes, "Count")?.as_i32(); // 数量
    for index in 1..=count { // 首个图表
        if let Ok(shape) = call(&shapes, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()) { if get(&shape, "HasChart").map(|v| v.as_bool()).unwrap_or(false) { return Ok(shape); } } // 命中
    }
    Err(ToolError::new("INVALID_ARGUMENT", "当前幻灯片没有图表")) // 无图表
}
fn insert(args: &Map<String, Value>) -> ComResult<Value> { // 插入图表
    let slide = slide_of(args)?; // 幻灯片
    let data = arg(args, "data").and_then(|v| v.as_array()).ok_or_else(|| ToolError::new("INVALID_ARGUMENT", "insert 需要二维数组 data"))?; // 数据
    if data.is_empty() { return Err(ToolError::new("INVALID_ARGUMENT", "图表 data 不能为空")); } // 空数据
    if data.len() > 100 { return Err(ToolError::new("INVALID_ARGUMENT", "图表数据超过 100 行")); } // 行数上限
    let shapes = get(&slide, "Shapes")?.as_dispatch()?; // 形状集
    let kind = chart_type(&str_arg(args, "type")); // 类型
    let shape = call(&shapes, "AddChart2", &[Variant::I4(-1), Variant::I4(kind), Variant::R4(f64_arg(args, "left", 60.0) as f32), Variant::R4(f64_arg(args, "top", 160.0) as f32), Variant::R4(f64_arg(args, "width", 480.0) as f32), Variant::R4(f64_arg(args, "height", 300.0) as f32)])?.as_dispatch()?; // 添加
    let chart = get(&shape, "Chart")?.as_dispatch()?; // 图表
    fill_chart(&chart, data, &str_arg(args, "title"))?; // 写数据
    Ok(json!({"inserted": true, "shapeName": get(&shape, "Name").map(|v| v.as_string()).unwrap_or_default(), "chartType": kind})) // 返回
}
fn update_data(args: &Map<String, Value>) -> ComResult<Value> { // 更新数据
    let slide = slide_of(args)?; // 幻灯片
    let shape = chart_shape(&slide, args)?; // 图表形状
    let chart = get(&shape, "Chart")?.as_dispatch()?; // 图表
    let data = arg(args, "data").and_then(|v| v.as_array()).ok_or_else(|| ToolError::new("INVALID_ARGUMENT", "update_data 需要二维数组 data"))?; // 数据
    let series = fill_chart(&chart, data, &str_arg(args, "title"))?; // 写入
    Ok(json!({"updated": true, "series": series})) // 返回
}
fn set_title(args: &Map<String, Value>) -> ComResult<Value> { // 设置标题
    let slide = slide_of(args)?; // 幻灯片
    let shape = chart_shape(&slide, args)?; // 图表形状
    let chart = get(&shape, "Chart")?.as_dispatch()?; // 图表
    let title = str_arg(args, "title"); // 标题
    set(&chart, "HasTitle", Variant::I4(if title.is_empty() { 0 } else { MSO_TRUE }))?; // 显示标题
    if !title.is_empty() { let obj = get(&chart, "ChartTitle")?.as_dispatch()?; set(&obj, "Text", Variant::Str(title.clone()))?; } // 文本
    Ok(json!({"applied": true, "title": title})) // 返回
}
fn set_type(args: &Map<String, Value>) -> ComResult<Value> { // 设置类型
    let slide = slide_of(args)?; // 幻灯片
    let shape = chart_shape(&slide, args)?; // 图表形状
    let chart = get(&shape, "Chart")?.as_dispatch()?; // 图表
    let kind = chart_type(&str_arg(args, "type")); // 类型
    set(&chart, "ChartType", Variant::I4(kind))?; // 设置
    Ok(json!({"applied": true, "chartType": kind})) // 返回
}
fn fill_chart(chart: &ComObj, data: &[Value], title: &str) -> ComResult<usize> { // 写系列
    let collection = call(chart, "SeriesCollection", &[])?.as_dispatch()?; // 系列集
    let count = get(&collection, "Count")?.as_i32(); // 旧系列
    let mut index = count; // 倒序清理
    while index >= 1 { if let Ok(series) = call(&collection, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()) { let _ = call(&series, "Delete", &[]); } index -= 1; } // 删除
    let columns = data.first().and_then(|row| row.as_array()).map(|row| row.len()).unwrap_or(0); // 列数
    let mut cursor = 1usize; // 跳过分类列
    while cursor < columns { // 逐系列
        let series = call(&collection, "NewSeries", &[])?.as_dispatch()?; // 新系列
        let name = data.first().and_then(|row| row.as_array()).and_then(|row| row.get(cursor)).map(json_text).unwrap_or_default(); // 名称
        set(&series, "Name", Variant::Str(name))?; // 名称
        let mut values = Vec::new(); // 值
        let mut categories = Vec::new(); // 分类
        for row in data.iter().skip(1) { // 跳过表头
            if let Some(cells) = row.as_array() { categories.push(json_text(cells.first().unwrap_or(&Value::Null))); values.push(number(cells.get(cursor).unwrap_or(&Value::Null))); } // 收集
        }
        set(&series, "Values", Variant::R8Array(values))?; // 值
        if cursor == 1 { set(&series, "XValues", Variant::Array(categories))?; } // 分类
        cursor += 1; // 下一列
    }
    if !title.is_empty() { // 标题
        if set(chart, "HasTitle", Variant::I4(MSO_TRUE)).is_ok() { if let Ok(obj) = get(chart, "ChartTitle").and_then(|v| v.as_dispatch()) { let _ = set(&obj, "Text", Variant::Str(title.to_string())); } } // 写标题
    }
    Ok(columns.saturating_sub(1)) // 返回系列数
}
fn json_text(value: &Value) -> String { match value { Value::String(text) => text.clone(), Value::Number(number) => number.to_string(), Value::Bool(flag) => flag.to_string(), _ => String::new() } } // JSON 转文本
fn number(value: &Value) -> f64 { match value { Value::Number(number) => number.as_f64().unwrap_or(0.0), Value::String(text) => text.trim().parse().unwrap_or(0.0), _ => 0.0 } } // JSON 转数值
#[allow(dead_code)] // 备用文本框
fn fallback(slide: &ComObj) -> ComResult<ComObj> { add_textbox(slide, 60.0, 160.0, 480.0, 120.0, "") } // 兜底