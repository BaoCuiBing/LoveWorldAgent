use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::constants::MSO_TRUE; // COM 布尔真
use crate::com::dispatch::{call, get, set, ComObj, ComResult, Variant}; // COM 工具
use crate::ppt::locator::{self, add_textbox, i32_arg, set_text, str_arg}; // 上下文与辅助
use crate::word::report::{err_json, ok_json, ToolError}; // 信封

pub fn run(args: &Map<String, Value>) -> Value { // ppt_write 5 action
    let action = str_arg(args, "action"); // 动作
    let result: ComResult<Value> = match action.as_str() { // 分派
        "title" => title(args), // 标题
        "text" => text(args), // 正文
        "bullets" => bullets(args), // 项目符号
        "markdown" => markdown(args), // Markdown
        "notes" => notes(args), // 备注
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 ppt_write action：{other}"))), // 未知
    };
    match result { Ok(value) => ok_json(value), Err(err) => err_json(&err) } // 返回
}
fn slide_of(args: &Map<String, Value>) -> ComResult<(locator::PresCtx, ComObj, i32)> { // 取幻灯片
    let ctx = locator::open(&str_arg(args, "path"))?; // 上下文
    let index = i32_arg(args, "slideIndex", 1).max(1); // 序号
    let slide = locator::slide(&ctx, index)?; // 幻灯片
    Ok((ctx, slide, index)) // 返回
}
fn title_shape(slide: &ComObj, text: &str) -> ComResult<ComObj> { // 标题形状（占位符优先，否则文本框）
    let shapes = get(slide, "Shapes")?.as_dispatch()?; // 形状集
    if let Ok(shape) = get(&shapes, "Title").and_then(|v| v.as_dispatch()) { set_text(&shape, text)?; return Ok(shape); } // 标题占位符
    add_textbox(slide, 40.0, 30.0, 620.0, 80.0, text) // 文本框兜底
}
fn body_shape(slide: &ComObj, text: &str) -> ComResult<ComObj> { // 正文形状
    let shapes = get(slide, "Shapes")?.as_dispatch()?; // 形状集
    if let Ok(placeholders) = get(&shapes, "Placeholders").and_then(|v| v.as_dispatch()) { // 占位符集
        let count = get(&placeholders, "Count").map(|v| v.as_i32()).unwrap_or(0); // 数量
        for index in 2..=count { // 跳过标题
            if let Ok(shape) = call(&placeholders, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()) { // 占位符
                if get(&shape, "HasTextFrame").map(|v| v.as_bool()).unwrap_or(false) { set_text(&shape, text)?; return Ok(shape); } // 命中
            }
        }
    }
    add_textbox(slide, 60.0, 150.0, 620.0, 320.0, text) // 文本框兜底
}
fn title(args: &Map<String, Value>) -> ComResult<Value> { // 写标题
    let (_ctx, slide, index) = slide_of(args)?; // 幻灯片
    let text = str_arg(args, "text"); // 文本
    let shape = title_shape(&slide, &text)?; // 形状
    Ok(json!({"written": true, "slideIndex": index, "shapeName": get(&shape, "Name").map(|v| v.as_string()).unwrap_or_default()})) // 返回
}
fn text(args: &Map<String, Value>) -> ComResult<Value> { // 写正文
    let (_ctx, slide, index) = slide_of(args)?; // 幻灯片
    let value = str_arg(args, "text"); // 文本
    let shape = body_shape(&slide, &value)?; // 形状
    Ok(json!({"written": true, "slideIndex": index, "shapeName": get(&shape, "Name").map(|v| v.as_string()).unwrap_or_default()})) // 返回
}
fn bullets(args: &Map<String, Value>) -> ComResult<Value> { // 项目符号
    let (_ctx, slide, index) = slide_of(args)?; // 幻灯片
    let items = locals(args); // 条目
    let shape = body_shape(&slide, &items.join("\r"))?; // 写文本
    let frame = get(&shape, "TextFrame")?.as_dispatch()?; // TextFrame
    let range = get(&frame, "TextRange")?.as_dispatch()?; // TextRange
    if let Ok(format) = get(&range, "ParagraphFormat").and_then(|v| v.as_dispatch()) { // 段落格式
        if let Ok(bullet) = get(&format, "Bullet").and_then(|v| v.as_dispatch()) { let _ = set(&bullet, "Visible", Variant::I4(MSO_TRUE)); } // 显示项目符号（不支持则忽略）
    }
    Ok(json!({"written": true, "slideIndex": index, "bullets": items.len()})) // 返回
}
fn markdown(args: &Map<String, Value>) -> ComResult<Value> { // Markdown
    let (_ctx, slide, index) = slide_of(args)?; // 幻灯片
    let source = str_arg(args, "markdown"); // 源
    let mut heading = String::new(); // 标题
    let mut body: Vec<String> = Vec::new(); // 正文
    for line in source.lines() { // 逐行
        if heading.is_empty() && line.starts_with("# ") { heading = line.trim_start_matches('#').trim().to_string(); } else if !line.trim().is_empty() { body.push(line.to_string()); } // 归一
    }
    if !heading.is_empty() { title_shape(&slide, &heading)?; } // 标题
    if !body.is_empty() { body_shape(&slide, &body.join("\r"))?; } // 正文
    Ok(json!({"written": true, "slideIndex": index, "heading": heading, "lines": body.len()})) // 返回
}
fn notes(args: &Map<String, Value>) -> ComResult<Value> { // 备注
    let (_ctx, slide, index) = slide_of(args)?; // 幻灯片
    let page = get(&slide, "NotesPage")?.as_dispatch()?; // 备注页
    let shapes = get(&page, "Shapes")?.as_dispatch()?; // 形状集
    let count = get(&shapes, "Count")?.as_i32(); // 数量
    let mut target: Option<ComObj> = None; // 备注体
    for cursor in 1..=count { // 逐个
        if let Ok(shape) = call(&shapes, "Item", &[Variant::I4(cursor)]).and_then(|v| v.as_dispatch()) { // 形状
            let kind = get(&shape, "PlaceholderFormat").ok().and_then(|v| v.as_dispatch().ok()).and_then(|format| get(&format, "Type").ok()).map(|v| v.as_i32()).unwrap_or(0); // 占位符类型
            if kind == 2 { target = Some(shape); break; } // 备注体
        }
    }
    let shape = target.ok_or_else(|| ToolError::new("INVALID_ARGUMENT", "备注页缺少备注占位符"))?; // 缺失
    set_text(&shape, &str_arg(args, "text"))?; // 写备注
    Ok(json!({"written": true, "slideIndex": index})) // 返回
}
fn locals(args: &Map<String, Value>) -> Vec<String> { // 读取 items 或按行拆分 text
    if let Some(items) = args.get("items").and_then(|v| v.as_array()) { return items.iter().map(|item| item.as_str().unwrap_or("").to_string()).filter(|item| !item.is_empty()).collect(); } // 数组
    str_arg(args, "text").lines().map(|line| line.trim().to_string()).filter(|line| !line.is_empty()).collect() // 按行
}