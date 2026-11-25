use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::dispatch::{call, get, set, ComObj, ComResult, Variant}; // COM 工具
use crate::com::document::active_document; // 文档上下文
use crate::com::constants::*; // 常量
use crate::word::paths::Paths; // 路径
use crate::word::report::{err_json, ok_json, ToolError}; // 信封
fn arg<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a Value> { args.get(key) } // 取参数
fn str_arg(args: &Map<String, Value>, key: &str) -> String { arg(args, key).and_then(|v| v.as_str()).unwrap_or("").to_string() } // 字符串
fn f64_arg(args: &Map<String, Value>, key: &str, d: f64) -> f64 { arg(args, key).and_then(|v| v.as_f64()).unwrap_or(d) } // 浮点
pub fn run(args: &Map<String, Value>) -> Value { // word_media 3 action
    let action = str_arg(args, "action"); // 动作
    let path = str_arg(args, "path"); // 路径
    let paths = Paths::load(); // 策略
    let ctx = match active_document(&path, &paths) { Ok(c) => c, Err(e) => return err_json(&e) }; // 取文档
    if let Err(err) = crate::com::document::ensure_writable(&ctx.app) { return err_json(&err); } // 对话框拦截
    if ctx.read_only() { return err_json(&ToolError::new("READ_ONLY", "文档为只读，无法插入媒体").recoverable("取消只读或另存为可写文档后重试")); } // 只读拦截
    let result: ComResult<Value> = match action.as_str() { // 分派
        "image" => image(&ctx, args, &paths), // 图片
        "chart" => chart(&ctx, args), // 图表
        "textbox" => textbox(&ctx, args), // 文本框
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 media action：{other}"))), // 未知
    };
    match result { Ok(v) => ok_json(v), Err(e) => err_json(&e) } // 返回
}
fn image(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>, paths: &Paths) -> ComResult<Value> { // 图片
    let image_path = str_arg(args, "imagePath"); // 图片路径
    let full = paths.resolve(&image_path)?; // 校验
    if !full.is_file() { return Err(ToolError::new("FILE_NOT_FOUND", format!("图片不存在：{}", full.display()))); } // 不存在
    let ext = full.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase(); // 扩展名
    if !["jpg", "jpeg", "png", "gif", "bmp"].contains(&ext.as_str()) { return Err(ToolError::new("INVALID_ARGUMENT", "仅支持 jpg/png/gif/bmp")); } // 类型
    let range = crate::com::document::main_body_end_range(&ctx.doc)?; // 主正文末尾（避开表格/页眉）
    let shapes = get(&ctx.doc, "InlineShapes")?.as_dispatch()?; // 集合
    let shape = call(&shapes, "AddPicture", &[Variant::Str(full.to_string_lossy().to_string()), Variant::Bool(false), Variant::Bool(true), Variant::Dispatch(range)])?.as_dispatch()?; // 添加
    if let Some(w) = arg(args, "width").and_then(|v| v.as_f64()) { set(&shape, "Width", Variant::R8(w.min(1000.0)))?; } // 宽
    if let Some(h) = arg(args, "height").and_then(|v| v.as_f64()) { set(&shape, "Height", Variant::R8(h.min(1000.0)))?; } // 高
    Ok(json!({"inserted": true})) // 返回
}
fn chart_type(name: &str) -> i32 { // 图表类型
    match name { "bar" => XL_BAR_CLUSTERED, "line" => XL_LINE, "pie" => XL_PIE, "area" => XL_AREA, _ => XL_COLUMN_CLUSTERED } // 映射
}
fn chart(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 图表
    let kind = chart_type(&str_arg(args, "type")); // 类型
    let data = arg(args, "data").and_then(|v| v.as_array()).ok_or_else(|| ToolError::new("INVALID_ARGUMENT", "data 必须是二维数组"))?; // 数据
    if data.is_empty() { return Err(ToolError::new("INVALID_ARGUMENT", "图表 data 不能为空")); } // 空数据
    if data.len() > 100 { return Err(ToolError::new("INVALID_ARGUMENT", "图表数据超过 100 行")); } // 限制
    let range = crate::com::document::main_body_end_range(&ctx.doc)?; // 主正文末尾（避开表格/页眉）
    let width = f64_arg(args, "width", 400.0); // 宽
    let height = f64_arg(args, "height", 250.0); // 高
    let shapes = get(&ctx.doc, "InlineShapes")?.as_dispatch()?; // 集合
    let shape = call(&shapes, "AddChart2", &[Variant::I4(-1), Variant::I4(kind), Variant::Dispatch(range)])?.as_dispatch()?; // 添加图表
    let _ = set(&shape, "Width", Variant::R4(width as f32)); // 宽
    let _ = set(&shape, "Height", Variant::R4(height as f32)); // 高
    let chart = match get(&shape, "Chart").and_then(|v| v.as_dispatch()) { Ok(chart) => chart, Err(err) => { let _ = call(&shape, "Delete", &[]); return Err(err); } }; // Chart
    match fill_chart(&chart, data, &str_arg(args, "title")) { // 写系列
        Ok(series) => Ok(json!({"inserted": true, "series": series})), // 成功
        Err(err) => { let _ = call(&shape, "Delete", &[]); Err(err) } // 失败清理半成品
    }
}
fn fill_chart(chart: &ComObj, data: &[Value], title: &str) -> ComResult<usize> { // 写图表系列
    let series_collection = call(chart, "SeriesCollection", &[])?.as_dispatch()?; // 系列集（方法调用）
    let count = get(&series_collection, "Count")?.as_i32(); // 数量
    let mut i = count; // 倒序清理
    while i >= 1 { // 遍历
        if let Ok(s) = call(&series_collection, "Item", &[Variant::I4(i)]).and_then(|v| v.as_dispatch()) { let _ = call(&s, "Delete", &[]); } // 删除
        i -= 1; // 前一
    }
    let ncols = data.first().and_then(|r| r.as_array()).map(|r| r.len()).unwrap_or(0); // 列数
    let mut c = 1usize; // 系列列（跳过第 0 列分类）
    while c < ncols { // 逐系列
        let series = call(&series_collection, "NewSeries", &[])?.as_dispatch()?; // 新系列
        let name = data.first().and_then(|r| r.as_array()).and_then(|r| r.get(c)).map(json_str).unwrap_or_default(); // 名称
        set(&series, "Name", Variant::Str(name))?; // 名称
        let mut values = Vec::new(); // 值
        let mut categories = Vec::new(); // 分类
        for row in data.iter().skip(1) { // 跳过表头
            if let Some(cells) = row.as_array() { // 行
                categories.push(json_str(cells.first().unwrap_or(&Value::Null))); // 分类
                let raw = cells.get(c).unwrap_or(&Value::Null); // 单元格
                values.push(match raw { Value::Number(n) => n.as_f64().unwrap_or(0.0), Value::String(s) => s.trim().parse::<f64>().unwrap_or(0.0), _ => 0.0 }); // 数值
            }
        }
        set(&series, "Values", Variant::R8Array(values))?; // 值（SAFEARRAY R8）
        if c == 1 { set(&series, "XValues", Variant::Array(categories))?; } // 分类（SAFEARRAY BSTR）
        c += 1; // 下一列
    }
    if !title.is_empty() { // 标题（可选，失败不阻断图表）
        if set(chart, "HasTitle", Variant::Bool(true)).is_ok() { // 显示标题
            if let Ok(title_obj) = get(chart, "ChartTitle").and_then(|v| v.as_dispatch()) { let _ = set(&title_obj, "Text", Variant::Str(title.to_string())); } // 写标题
        }
    }
    Ok(ncols.saturating_sub(1)) // 返回系列数
}
fn textbox(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 文本框
    let orientation = if str_arg(args, "orientation") == "vertical" { MSO_TEXT_ORIENTATION_VERTICAL } else { MSO_TEXT_ORIENTATION_HORIZONTAL }; // 方向
    let left = f64_arg(args, "positionLeft", 50.0); // 左
    let top = f64_arg(args, "positionTop", 50.0); // 上
    let width = f64_arg(args, "width", 200.0); // 宽
    let height = f64_arg(args, "height", 100.0); // 高
    let shapes = get(&ctx.doc, "Shapes")?.as_dispatch()?; // 集合
    let anchor = crate::com::document::main_body_end_range(&ctx.doc)?; // 主正文末尾锚点
    let shape = call(&shapes, "AddTextbox", &[Variant::I4(orientation), Variant::R8(left), Variant::R8(top), Variant::R8(width), Variant::R8(height), Variant::Dispatch(anchor)])?.as_dispatch()?; // 添加
    let frame = get(&shape, "TextFrame")?.as_dispatch()?; // 文本框
    let range = get(&frame, "TextRange")?.as_dispatch()?; // 文本区
    set(&range, "Text", Variant::Str(str_arg(args, "text")))?; // 写文本
    Ok(json!({"inserted": true})) // 返回
}
fn json_str(value: &Value) -> String { // JSON 值转字符串
    match value { Value::String(s) => s.clone(), Value::Number(n) => n.to_string(), Value::Bool(b) => b.to_string(), _ => String::new() } // 分派
}
