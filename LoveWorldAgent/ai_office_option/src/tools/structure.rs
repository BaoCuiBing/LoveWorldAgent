use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::dispatch::{get_args, call, get, set, ComObj, ComResult, Variant}; // COM 工具
use crate::com::document::active_document; // 文档上下文
use crate::word::paths::Paths; // 路径
use crate::word::report::{err_json, ok_json, ToolError}; // 信封
fn arg<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a Value> { args.get(key) } // 取参数
fn str_arg(args: &Map<String, Value>, key: &str) -> String { arg(args, key).and_then(|v| v.as_str()).unwrap_or("").to_string() } // 字符串
fn i32_arg(args: &Map<String, Value>, key: &str, d: i32) -> i32 { arg(args, key).and_then(|v| v.as_i64()).unwrap_or(d as i64) as i32 } // 整数
pub fn run(args: &Map<String, Value>) -> Value { // word_structure 10 action
    let action = str_arg(args, "action"); // 动作
    let path = str_arg(args, "path"); // 路径
    let paths = Paths::load(); // 策略
    let ctx = match active_document(&path, &paths) { Ok(c) => c, Err(e) => return err_json(&e) }; // 取文档
    if let Err(err) = crate::com::document::ensure_writable(&ctx.app) { return err_json(&err); } // 对话框拦截
    let saved = selection_bounds(&ctx).ok(); // 保存主正文选区
    let result: ComResult<Value> = match action.as_str() { // 分派
        "hyperlink" => hyperlink(&ctx, args), // 链接
        "footnote" => footnote(&ctx, args), // 脚注
        "comment" => comment(&ctx, args), // 批注
        "bookmark" => bookmark(&ctx, args), // 书签
        "toc" => toc(&ctx, args), // 目录
        "page_region" => page_region(&ctx, args), // 页眉页脚
        "page_numbers" => page_numbers(&ctx, args), // 页码
        "watermark" => watermark(&ctx, args), // 水印
        "columns" => columns(&ctx, args), // 分栏
        "section_break" => section_break(&ctx, args), // 分节符
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 structure action：{other}"))), // 未知
    };
    if matches!(action.as_str(), "page_region" | "page_numbers" | "watermark") { // 页眉/页脚/水印后恢复主正文
        if let Some((start, end)) = saved { let _ = restore_selection(&ctx, start, end); } // 恢复
    }
    match result { Ok(v) => ok_json(v), Err(e) => err_json(&e) } // 返回
}
fn selection_bounds(ctx: &crate::com::document::DocCtx) -> ComResult<(i32, i32)> { // 当前选区
    let app = get(&ctx.doc, "Application")?.as_dispatch()?; // Application
    let sel = get(&app, "Selection")?.as_dispatch()?; // Selection
    Ok((get(&sel, "Start")?.as_i32(), get(&sel, "End")?.as_i32())) // 返回
}
fn restore_selection(ctx: &crate::com::document::DocCtx, start: i32, end: i32) -> ComResult<()> { // 恢复到主正文
    let app = get(&ctx.doc, "Application")?.as_dispatch()?; // Application
    let sel = get(&app, "Selection")?.as_dispatch()?; // Selection
    call(&sel, "SetRange", &[Variant::I4(start), Variant::I4(end)])?; // 回正文
    Ok(()) // 完成
}
fn anchor(ctx: &crate::com::document::DocCtx) -> ComResult<ComObj> { // 取插入定位 Range
    let app = get(&ctx.doc, "Application")?.as_dispatch()?; // Application
    let sel = get(&app, "Selection")?.as_dispatch()?; // Selection
    get(&sel, "Range")?.as_dispatch() // Range
}
fn hyperlink(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 超链接
    let range = anchor(ctx)?; // Range
    let text = str_arg(args, "text"); // 文本
    let address = str_arg(args, "address"); // 地址
    if address.trim().is_empty() { return Err(ToolError::new("INVALID_ARGUMENT", "address 不能为空")); } // 校验

    let links = get(&ctx.doc, "Hyperlinks")?.as_dispatch()?; // 集合
    call(&links, "Add", &[Variant::Dispatch(range), Variant::Str(address), Variant::Str(String::new()), Variant::Str(String::new()), Variant::Str(text)])?; // 添加
    Ok(json!({"added": true})) // 返回
}
fn footnote(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 脚注
    let range = anchor(ctx)?; // Range
    let notes = get(&ctx.doc, "Footnotes")?.as_dispatch()?; // 集合
    call(&notes, "Add", &[Variant::Dispatch(range), Variant::Str(String::new()), Variant::Str(str_arg(args, "text"))])?; // 添加
    Ok(json!({"added": true})) // 返回
}
fn comment(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 批注
    let range = anchor(ctx)?; // Range
    let comments = get(&ctx.doc, "Comments")?.as_dispatch()?; // 集合
    call(&comments, "Add", &[Variant::Dispatch(range), Variant::Str(str_arg(args, "text"))])?; // 添加
    Ok(json!({"added": true})) // 返回
}
fn bookmark(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 书签
    let range = anchor(ctx)?; // Range
    let bookmarks = get(&ctx.doc, "Bookmarks")?.as_dispatch()?; // 集合
    call(&bookmarks, "Add", &[Variant::Str(str_arg(args, "name")), Variant::Dispatch(range)])?; // 添加
    Ok(json!({"added": true})) // 返回
}
fn toc(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 目录
    let range = anchor(ctx)?; // Range
    let levels = i32_arg(args, "levels", 3).clamp(1, 3); // 级别
    let tocs = get(&ctx.doc, "TablesOfContents")?.as_dispatch()?; // 集合
    call(&tocs, "Add", &[Variant::Dispatch(range), Variant::Bool(true), Variant::I4(1), Variant::I4(levels)])?; // 添加
    Ok(json!({"added": true})) // 返回
}
fn header_footer(ctx: &crate::com::document::DocCtx, section: i32, target: &str) -> ComResult<ComObj> { // 取页眉页脚
    let sections = get(&ctx.doc, "Sections")?.as_dispatch()?; // 集合
    let sec = call(&sections, "Item", &[Variant::I4(section)])?.as_dispatch()?; // 节
    let collection = if target == "footer" { get(&sec, "Footers")?.as_dispatch()? } else { get(&sec, "Headers")?.as_dispatch()? }; // 集合
    call(&collection, "Item", &[Variant::I4(1)])?.as_dispatch() // 主页眉
}
fn page_region(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 页眉页脚文本
    let target = str_arg(args, "target"); // 目标
    let section = i32_arg(args, "sectionIndex", 1).max(1); // 节
    let hf = header_footer(ctx, section, &target)?; // 页眉页脚
    let range = get(&hf, "Range")?.as_dispatch()?; // Range
    set(&range, "Text", Variant::Str(str_arg(args, "text")))?; // 写文本
    Ok(json!({"applied": true})) // 返回
}
fn page_numbers(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 页码
    let target = str_arg(args, "target"); // 目标
    let section = i32_arg(args, "sectionIndex", 1).max(1); // 节
    let hf = header_footer(ctx, section, &target)?; // 页眉页脚
    let numbers = get(&hf, "PageNumbers")?.as_dispatch()?; // 页码集
    call(&numbers, "Add", &[Variant::I4(1), Variant::Bool(true)])?; // 添加
    if let Some(start) = arg(args, "startNumber").and_then(|v| v.as_i64()) { set(&numbers, "StartingNumber", Variant::I4(start as i32))?; } // 起始
    Ok(json!({"added": true})) // 返回
}
fn watermark(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 水印
    let sections = get(&ctx.doc, "Sections")?.as_dispatch()?; // 集合
    let sec = call(&sections, "Item", &[Variant::I4(1)])?.as_dispatch()?; // 节
    let headers = get(&sec, "Headers")?.as_dispatch()?; // 页眉集
    let header = call(&headers, "Item", &[Variant::I4(1)])?.as_dispatch()?; // 页眉
    let shapes = get(&header, "Shapes")?.as_dispatch()?; // 形状集
    if arg(args, "remove").and_then(|v| v.as_bool()).unwrap_or(false) { // 移除
        let count = get(&shapes, "Count")?.as_i32(); // 数量
        let mut i = count; // 倒序
        while i >= 1 { // 遍历
            if let Ok(shape) = call(&shapes, "Item", &[Variant::I4(i)]).and_then(|v| v.as_dispatch()) { call(&shape, "Delete", &[]).ok(); } // 删除
            i -= 1; // 前一
        }
    } else { // 添加
        let size = i32_arg(args, "fontSize", 48); // 字号
        call(&shapes, "AddTextEffect", &[Variant::I4(0), Variant::Str(str_arg(args, "text")), Variant::Str("微软雅黑".into()), Variant::I4(size), Variant::Bool(false), Variant::Bool(false), Variant::I4(0), Variant::I4(0)])?; // 添加
    }
    Ok(json!({"applied": true})) // 返回
}
fn columns(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 分栏
    let section = i32_arg(args, "sectionIndex", 1).max(1); // 节
    let sections = get(&ctx.doc, "Sections")?.as_dispatch()?; // 集合
    let sec = call(&sections, "Item", &[Variant::I4(section)])?.as_dispatch()?; // 节
    let setup = get(&sec, "PageSetup")?.as_dispatch()?; // 页面
    let text_cols = get(&setup, "TextColumns")?.as_dispatch()?; // 分栏集
    call(&text_cols, "SetCount", &[Variant::I4(i32_arg(args, "count", 1).clamp(1, 4))])?; // 数量
    Ok(json!({"applied": true})) // 返回
}
fn section_break(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 分节符
    let app = get(&ctx.doc, "Application")?.as_dispatch()?; // Application
    let sel = get(&app, "Selection")?.as_dispatch()?; // Selection
    let value = match str_arg(args, "type").as_str() { "continuous" => 3, "evenPage" => 4, "oddPage" => 5, _ => 2 }; // 类型映射
    call(&sel, "InsertBreak", &[Variant::I4(value)])?; // 插入分节
    Ok(json!({"applied": true})) // 返回
}
