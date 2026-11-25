use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::constants::*; // 常量
use crate::com::dispatch::{get_args, call, get, set, ComObj, Variant}; // COM 工具
use crate::com::document::{active_document, DocCtx}; // 文档上下文
use crate::word::locator::{locate_range, Locator}; // 定位
use crate::word::paths::Paths; // 路径
use crate::word::report::{err_json, ok_json, ToolError}; // 信封
fn arg<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a Value> { args.get(key) } // 取参数
fn str_arg(args: &Map<String, Value>, key: &str) -> String { arg(args, key).and_then(|v| v.as_str()).unwrap_or("").to_string() } // 字符串
fn i32_arg(args: &Map<String, Value>, key: &str, default: i32) -> i32 { arg(args, key).and_then(|v| v.as_i64()).unwrap_or(default as i64) as i32 } // 整数
pub fn run(args: &Map<String, Value>) -> Value { // word_read 10 target
    let target = str_arg(args, "target"); // 目标
    let path = str_arg(args, "path"); // 路径
    if path.is_empty() { return err_json(&ToolError::new("INVALID_ARGUMENT", "word_read 必须传 path")); } // 校验
    let paths = Paths::load(); // 策略
    let ctx = match active_document(&path, &paths) { Ok(c) => c, Err(e) => return err_json(&e) }; // 取文档
    match target.as_str() { // 分派
        "" | "text" => read_text(&ctx, args), // 正文
        "paragraph" => read_paragraph(&ctx, args), // 段落
        "table" => read_table(&ctx, args), // 表格
        "comments" => read_comments(&ctx), // 批注
        "bookmarks" => read_bookmarks(&ctx), // 书签
        "lists" => read_lists(&ctx), // 列表
        "sections" => read_sections(&ctx), // 节
        "styles" => read_styles(&ctx), // 样式
        "cursor" => read_cursor(&ctx), // 光标
        "locate" => read_locate(&ctx, args), // 定位
        other => err_json(&ToolError::new("INVALID_ARGUMENT", format!("未知 target：{other}"))), // 未知
    }
}
fn read_text(ctx: &DocCtx, args: &Map<String, Value>) -> Value { // 正文按字符分页读取
    let offset = i32_arg(args, "offset", 0); // 偏移
    if offset < 0 { return err_json(&ToolError::new("INVALID_ARGUMENT", "offset 必须 >= 0")); } // 校验
    let limit = i32_arg(args, "limit", 200000); // 上限
    if limit < 1 || limit > 200000 { return err_json(&ToolError::new("INVALID_ARGUMENT", "limit 需 1~200000")); } // 校验
    let content = match get(&ctx.doc, "Content").and_then(|v| v.as_dispatch()) { Ok(c) => c, Err(e) => return err_json(&e) }; // 正文
    let raw = get(&content, "Text").map(|v| v.as_string()).unwrap_or_default(); // 全文
    let chars: Vec<char> = raw.chars().collect(); // 字符表
    let total = chars.len() as i32; // 总字符数
    if offset as usize >= chars.len() { return ok_json(json!({"text": "", "chars": 0, "total": total, "truncated": false, "nextOffset": null})); } // 越界
    let begin = offset as usize; // 起点
    let end = (begin + limit as usize).min(chars.len()); // 终点
    let text: String = chars[begin..end].iter().collect::<String>().replace('\r', "\n"); // 切片
    let count = (end - begin) as i32; // 字符数
    ok_json(json!({"text": text, "chars": count, "total": total, "truncated": end < chars.len(), "nextOffset": if end < chars.len() { json!(end) } else { Value::Null }})) // 返回
}
fn read_paragraph(ctx: &DocCtx, args: &Map<String, Value>) -> Value { // 段落读取
    let paras = get(&ctx.doc, "Paragraphs").and_then(|v| v.as_dispatch()); // 集合
    let Ok(paras) = paras else { return err_json(&ToolError::new("NO_DOCUMENT", "无法读取段落")); }; // 失败
    let count = get(&paras, "Count").map(|v| v.as_i32()).unwrap_or(0); // 数量
    let index = if arg(args, "locator").is_some() { // 定位器
        let loc = Locator::from_json(arg(args, "locator")); // 解析
        match locate_range(&ctx.doc, &loc) { Ok(r) if r.found => r.paragraph_index.max(1), Ok(_) => 0, Err(e) => return err_json(&e) } // 命中
    } else { i32_arg(args, "index", 1) }; // 序号
    if index < 1 || index > count { return err_json(&ToolError::new("INVALID_ARGUMENT", format!("paragraph={index} > paragraphs={count}"))); } // 越界报错
    let para = call(&paras, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()); // 段落
    let Ok(para) = para else { return err_json(&ToolError::new("NO_DOCUMENT", "段落不存在")); }; // 失败
    let range = get(&para, "Range").and_then(|v| v.as_dispatch()); // Range
    let Ok(range) = range else { return err_json(&ToolError::new("NO_DOCUMENT", "段落 Range 无效")); }; // 失败
    let text = get(&range, "Text").map(|v| v.as_string()).unwrap_or_default().trim().to_string(); // 文本
    let style = get(&para, "Style").and_then(|v| v.as_dispatch()).and_then(|s| get(&s, "NameLocal")).map(|v| v.as_string()).unwrap_or_default(); // 样式
    let outline = get(&para, "OutlineLevel").map(|v| v.as_i32()).unwrap_or(10); // 大纲
    let start = get(&range, "Start").map(|v| v.as_i32()).unwrap_or(0); // 起点
    let end = get(&range, "End").map(|v| v.as_i32()).unwrap_or(0); // 终点
    ok_json(json!({"index": index, "text": text, "style": style, "outlineLevel": outline, "range": {"start": start, "end": end}})) // 返回
}
fn read_table(ctx: &DocCtx, args: &Map<String, Value>) -> Value { // 表格读取
    let index = i32_arg(args, "index", 1).max(1); // 序号
    let tables = get(&ctx.doc, "Tables").and_then(|v| v.as_dispatch()); // 集合
    let Ok(tables) = tables else { return err_json(&ToolError::new("NO_DOCUMENT", "无法读取表格")); }; // 失败
    let count = get(&tables, "Count").map(|v| v.as_i32()).unwrap_or(0); // 数量
    if index > count { return err_json(&ToolError::new("INVALID_ARGUMENT", format!("table={index} > tables={count}"))); } // 越界
    let table = call(&tables, "Item", &[Variant::I4(index)]).and_then(|v| v.as_dispatch()); // 表格
    let Ok(table) = table else { return err_json(&ToolError::new("NO_DOCUMENT", "表格不存在")); }; // 失败
    let rows = get(&table, "Rows").and_then(|v| v.as_dispatch()).and_then(|r| get(&r, "Count")).map(|v| v.as_i32()).unwrap_or(0); // 行
    let cols = get(&table, "Columns").and_then(|v| v.as_dispatch()).and_then(|c| get(&c, "Count")).map(|v| v.as_i32()).unwrap_or(0); // 列
    let mut data = Vec::new(); // 数据
    let mut r = 1; // 行
    while r <= rows { // 逐行
        let mut row = Vec::new(); // 行数据
        let mut c = 1; // 列
        while c <= cols { // 逐列
            let cell = call(&table, "Cell", &[Variant::I4(r), Variant::I4(c)]).and_then(|v| v.as_dispatch()); // 单元格
            let text = cell.and_then(|cell| get(&cell, "Range").and_then(|v| v.as_dispatch()).and_then(|rg| get(&rg, "Text"))).map(|v| clean_cell(&v.as_string())).unwrap_or_default(); // 文本
            row.push(json!(text)); // 收集
            c += 1; // 下一位
        }
        data.push(json!(row)); // 收集行
        r += 1; // 下一行
    }
    ok_json(json!({"rows": rows, "cols": cols, "data": data})) // 返回
}
fn clean_cell(text: &str) -> String { // 去单元格结束符
    text.trim_end_matches(['\r', '\u{7}']).to_string() // 清理
}
fn read_comments(ctx: &DocCtx) -> Value { // 批注读取
    let comments = get(&ctx.doc, "Comments").and_then(|v| v.as_dispatch()); // 集合
    let Ok(comments) = comments else { return ok_json(json!({"comments": []})); }; // 空
    let count = get(&comments, "Count").map(|v| v.as_i32()).unwrap_or(0); // 数量
    let mut out = Vec::new(); // 结果
    let mut i = 1; // 遍历
    while i <= count { // 逐条
        if let Ok(c) = call(&comments, "Item", &[Variant::I4(i)]).and_then(|v| v.as_dispatch()) { // 批注
            let author = get(&c, "Author").map(|v| v.as_string()).unwrap_or_default(); // 作者
            let text = get(&c, "Range").and_then(|v| v.as_dispatch()).and_then(|r| get(&r, "Text")).map(|v| v.as_string()).unwrap_or_default(); // 文本
            out.push(json!({"index": i, "author": author, "text": text, "scope": ""})); // 收集
        }
        i += 1; // 下一条
    }
    ok_json(json!({"comments": out})) // 返回
}
fn read_bookmarks(ctx: &DocCtx) -> Value { // 书签读取
    let bookmarks = get(&ctx.doc, "Bookmarks").and_then(|v| v.as_dispatch()); // 集合
    let Ok(bookmarks) = bookmarks else { return ok_json(json!({"bookmarks": []})); }; // 空
    let count = get(&bookmarks, "Count").map(|v| v.as_i32()).unwrap_or(0); // 数量
    let mut out = Vec::new(); // 结果
    let mut i = 1; // 遍历
    while i <= count { // 逐条
        if let Ok(b) = call(&bookmarks, "Item", &[Variant::I4(i)]).and_then(|v| v.as_dispatch()) { // 书签
            let name = get(&b, "Name").map(|v| v.as_string()).unwrap_or_default(); // 名称
            let start = get(&b, "Start").map(|v| v.as_i32()).unwrap_or(0); // 起点
            let end = get(&b, "End").map(|v| v.as_i32()).unwrap_or(0); // 终点
            out.push(json!({"name": name, "start": start, "end": end})); // 收集
        }
        i += 1; // 下一条
    }
    ok_json(json!({"bookmarks": out})) // 返回
}
fn read_lists(ctx: &DocCtx) -> Value { // 列表读取
    let lists = get(&ctx.doc, "Lists").and_then(|v| v.as_dispatch()); // 集合
    let Ok(lists) = lists else { return ok_json(json!({"lists": []})); }; // 空
    let count = get(&lists, "Count").map(|v| v.as_i32()).unwrap_or(0); // 数量
    let mut out = Vec::new(); // 结果
    let mut i = 1; // 遍历
    while i <= count { // 逐个
        if let Ok(list) = call(&lists, "Item", &[Variant::I4(i)]).and_then(|v| v.as_dispatch()) { // 列表
            let paragraphs = get(&list, "ListParagraphs").and_then(|v| v.as_dispatch()); // 段落
            if let Ok(paras) = paragraphs { // 成功
                let pcount = get(&paras, "Count").map(|v| v.as_i32()).unwrap_or(0); // 数量
                let mut p = 1; // 遍历
                while p <= pcount { // 逐段
                    if let Ok(para) = call(&paras, "Item", &[Variant::I4(p)]).and_then(|v| v.as_dispatch()) { // 段落
                        let text = get(&para, "Range").and_then(|v| v.as_dispatch()).and_then(|r| get(&r, "Text")).map(|v| v.as_string()).unwrap_or_default(); // 文本
                        let level = get(&para, "Range").and_then(|v| v.as_dispatch()).and_then(|r| get(&r, "ListFormat")).and_then(|v| v.as_dispatch()).and_then(|lf| get(&lf, "ListLevelNumber")).map(|v| v.as_i32()).unwrap_or(1); // 级别
                        out.push(json!({"index": i, "level": level, "text": text.trim(), "listString": ""})); // 收集
                    }
                    p += 1; // 下一段
                }
            }
        }
        i += 1; // 下一个
    }
    ok_json(json!({"lists": out})) // 返回
}
fn number_of(obj: &crate::com::dispatch::ComObj, name: &str) -> f64 { // 数值属性（PROPERTYGET 为 0 时回退 METHOD）
    let value = get(obj, name).map(|v| v.as_f64()).unwrap_or(0.0); // 属性读
    if value != 0.0 { return value; } // 非零直接返回
    crate::com::dispatch::call(obj, name, &[]).map(|v| v.as_f64()).unwrap_or(0.0) // 方法回退
}
fn int_of(obj: &crate::com::dispatch::ComObj, name: &str) -> i32 { // 整数属性
    let value = get(obj, name).map(|v| v.as_i32()).unwrap_or(0); // 属性读
    if value != 0 { return value; } // 非零直接返回
    crate::com::dispatch::call(obj, name, &[]).map(|v| v.as_i32()).unwrap_or(0) // 方法回退
}
fn read_sections(ctx: &DocCtx) -> Value { // 节读取
    let sections = get(&ctx.doc, "Sections").and_then(|v| v.as_dispatch()); // 集合
    let Ok(sections) = sections else { return ok_json(json!({"sections": []})); }; // 空
    let count = get(&sections, "Count").map(|v| v.as_i32()).unwrap_or(0); // 数量
    crate::log_info!("read", "sections count={}", count); // 诊断
    let mut out = Vec::new(); // 结果
    let mut i = 1; // 遍历
    while i <= count { // 逐个
        if let Ok(sec) = call(&sections, "Item", &[Variant::I4(i)]).and_then(|v| v.as_dispatch()) { // 节
            if let Ok(setup) = get(&sec, "PageSetup").and_then(|v| v.as_dispatch()) { // 页面设置
                crate::log_info!("read", "PageWidth variant={:?}", get(&setup, "PageWidth")); // 诊断
                let page_width = number_of(&setup, "PageWidth"); // 页宽
                let page_height = number_of(&setup, "PageHeight"); // 页高
                let orientation = int_of(&setup, "Orientation"); // 方向
                let top = number_of(&setup, "TopMargin"); // 上边距
                let bottom = number_of(&setup, "BottomMargin"); // 下
                let left = number_of(&setup, "LeftMargin"); // 左
                let right = number_of(&setup, "RightMargin"); // 右
                out.push(json!({"index": i, "pageSetup": {"pageWidth": page_width, "pageHeight": page_height, "orientation": orientation, "topMargin": top, "bottomMargin": bottom, "leftMargin": left, "rightMargin": right}, "headerLinked": true, "footerLinked": true})); // 收集
            }
        }
        i += 1; // 下一个
    }
    ok_json(json!({"sections": out})) // 返回
}
fn read_styles(ctx: &DocCtx) -> Value { // 样式读取
    let styles = get(&ctx.doc, "Styles").and_then(|v| v.as_dispatch()); // 集合
    let Ok(styles) = styles else { return ok_json(json!({"styles": []})); }; // 空
    let count = get(&styles, "Count").map(|v| v.as_i32()).unwrap_or(0).min(300); // 数量（限流）
    let mut out = Vec::new(); // 结果
    let mut i = 1; // 遍历
    while i <= count { // 逐个
        if let Ok(s) = call(&styles, "Item", &[Variant::I4(i)]).and_then(|v| v.as_dispatch()) { // 样式
            let name = get(&s, "NameLocal").map(|v| v.as_string()).unwrap_or_default(); // 名称
            let kind = get(&s, "Type").map(|v| v.as_i32()).unwrap_or(0); // 类型
            let builtin = get(&s, "BuiltIn").map(|v| v.as_bool()).unwrap_or(false); // 内置
            let in_use = get(&s, "InUse").map(|v| v.as_bool()).unwrap_or(false); // 使用中
            out.push(json!({"name": name, "type": kind, "builtin": builtin, "inUse": in_use})); // 收集
        }
        i += 1; // 下一个
    }
    ok_json(json!({"styles": out})) // 返回
}
fn read_cursor(ctx: &DocCtx) -> Value { // 光标读取
    let app = get(&ctx.doc, "Application").and_then(|v| v.as_dispatch()); // Application
    let Ok(app) = app else { return err_json(&ToolError::new("WORD_GONE", "无法读取 Application")); }; // 失败
    let sel = get(&app, "Selection").and_then(|v| v.as_dispatch()); // Selection
    let Ok(sel) = sel else { return err_json(&ToolError::new("WORD_GONE", "无法读取 Selection")); }; // 失败
    let start = get(&sel, "Start").map(|v| v.as_i32()).unwrap_or(0); // 起点
    let end = get(&sel, "End").map(|v| v.as_i32()).unwrap_or(0); // 终点
    let text = get(&sel, "Text").map(|v| v.as_string()).unwrap_or_default(); // 文本
    let tables = get(&sel, "Tables").and_then(|v| v.as_dispatch()).and_then(|t| get(&t, "Count")).map(|v| v.as_i32()).unwrap_or(0); // 表
    let story = get(&sel, "StoryType").map(|v| v.as_i32()).unwrap_or(1); // 故事类型
    let page = call(&sel, "Information", &[Variant::I4(WD_ACTIVE_END_PAGE_NUMBER)]).map(|v| v.as_i32()).unwrap_or(1); // 页码
    ok_json(json!({"start": start, "end": end, "text": text, "inTable": tables > 0, "inHeaderFooter": story != 1, "page": page})) // 返回
}
fn read_locate(ctx: &DocCtx, args: &Map<String, Value>) -> Value { // 语义定位
    let loc = Locator::from_json(arg(args, "locator")); // 解析
    match locate_range(&ctx.doc, &loc) { // 定位
        Ok(r) => { let table_index = if loc.by == "table" { loc.occurrence as i32 } else { 0 }; ok_json(json!({"found": r.found, "paragraphIndex": r.paragraph_index, "headingContext": r.heading_context, "tableIndex": table_index, "page": 1, "start": r.start, "end": r.end})) }, // 成功
        Err(err) => err_json(&err), // 失败
    }
}
