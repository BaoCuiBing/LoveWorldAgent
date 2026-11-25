use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::dispatch::{call, call_named, get, set, ComObj, ComResult, Variant}; // COM 工具
use crate::com::document::{active_document, paragraph_index_at}; // 文档上下文
use crate::com::constants::*; // 常量
use crate::word::locator::{locate_range, Locator}; // 定位
use crate::word::paths::Paths; // 路径
use crate::word::report::{err_json, ok_json, ToolError}; // 信封
fn arg<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a Value> { args.get(key) } // 取参数
fn str_arg(args: &Map<String, Value>, key: &str) -> String { arg(args, key).and_then(|v| v.as_str()).unwrap_or("").to_string() } // 字符串
fn i32_arg(args: &Map<String, Value>, key: &str, d: i32) -> i32 { arg(args, key).and_then(|v| v.as_i64()).unwrap_or(d as i64) as i32 } // 整数
fn selection(ctx: &crate::com::document::DocCtx) -> ComResult<ComObj> { // 取 Selection
    let app = get(&ctx.doc, "Application")?.as_dispatch()?; // Application
    get(&app, "Selection")?.as_dispatch() // Selection
}
pub fn find(args: &Map<String, Value>) -> Value { // word_find 8 action
    let action = str_arg(args, "action"); // 动作
    let path = str_arg(args, "path"); // 路径
    let paths = Paths::load(); // 策略
    let ctx = match active_document(&path, &paths) { Ok(c) => c, Err(e) => return err_json(&e) }; // 取文档
    let result: ComResult<Value> = match action.as_str() { // 分派
        "find" => do_find(&ctx, args), // 查找
        "replace" => replace(&ctx, args), // 替换
        "goto" => goto(&ctx, args), // 跳转
        "goto_paragraph" => goto_paragraph(&ctx, args), // 段落跳转
        "select_all" => select_all(&ctx), // 全选
        "select_text" => select_text(&ctx, args), // 选文本
        "select_current" => select_current(&ctx, args), // 选当前
        "select_at" => select_at(&ctx, args), // 定位选
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 find action：{other}"))), // 未知
    };
    match result { Ok(v) => ok_json(v), Err(e) => err_json(&e) } // 返回
}
fn do_find(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 查找
    let content = get(&ctx.doc, "Content")?.as_dispatch()?; // 正文
    let finder = get(&content, "Find")?.as_dispatch()?; // Find
    set(&finder, "Text", Variant::Str(str_arg(args, "findText")))?; // 文本
    set(&finder, "MatchCase", Variant::Bool(arg(args, "matchCase").and_then(|v| v.as_bool()).unwrap_or(false)))?; // 区分大小写
    set(&finder, "MatchWholeWord", Variant::Bool(arg(args, "matchWholeWord").and_then(|v| v.as_bool()).unwrap_or(false)))?; // 全字
    set(&finder, "Forward", Variant::Bool(str_arg(args, "direction") != "backward"))?; // 方向
    set(&finder, "Wrap", Variant::I4(if arg(args, "wrap").and_then(|v| v.as_bool()).unwrap_or(true) { WD_FIND_CONTINUE } else { WD_FIND_STOP }))?; // 环绕
    set(&finder, "MatchWildcards", Variant::Bool(false))?; // 关闭通配符
    let named = [("FindText", Variant::Str(str_arg(args, "findText"))), ("MatchCase", Variant::Bool(arg(args, "matchCase").and_then(|v| v.as_bool()).unwrap_or(false))), ("MatchWholeWord", Variant::Bool(arg(args, "matchWholeWord").and_then(|v| v.as_bool()).unwrap_or(false))), ("Forward", Variant::Bool(str_arg(args, "direction") != "backward")), ("Wrap", Variant::I4(if arg(args, "wrap").and_then(|v| v.as_bool()).unwrap_or(true) { WD_FIND_CONTINUE } else { WD_FIND_STOP }))]; // 命名参数
    let mut found = call_named(&finder, "Execute", &named, &[]).map(|value| value.as_bool()).unwrap_or(false); // 命名参数优先
    if !found { found = call(&finder, "Execute", &[])?.as_bool(); } // 未命中回退属性
    if found { // 命中
        let target = get(&finder, "Parent").and_then(|v| v.as_dispatch()).unwrap_or_else(|_| content.clone()); // Find.Parent 为命中 Range
        let start = get(&target, "Start")?.as_i32(); // 命中起点
        let end = get(&target, "End")?.as_i32(); // 命中终点
        let page = call(&target, "Information", &[Variant::I4(WD_ACTIVE_END_PAGE_NUMBER)]).map(|v| v.as_i32()).unwrap_or(1); // 页码
        Ok(json!({"found": true, "start": start, "end": end, "paragraphIndex": paragraph_index_at(&ctx.doc, start), "page": page})) // 返回
    } else { Ok(json!({"found": false})) } // 未命中
}
fn replace(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 替换（命名参数一次执行，避免循环阻塞）
    let find = str_arg(args, "findText"); // 查找
    let replacement = str_arg(args, "replaceWith"); // 替换
    let all = arg(args, "replaceAll").and_then(|v| v.as_bool()).unwrap_or(true); // 全部
    let match_case = arg(args, "matchCase").and_then(|v| v.as_bool()).unwrap_or(false); // 区分大小写
    let whole_word = arg(args, "matchWholeWord").and_then(|v| v.as_bool()).unwrap_or(false); // 全字
    let content = get(&ctx.doc, "Content")?.as_dispatch()?; // 正文
    let text = get(&content, "Text").map(|v| v.as_string()).unwrap_or_default(); // 替换前文本
    let count = if match_case { text.matches(&find).count() } else { text.to_lowercase().matches(&find.to_lowercase()).count() }; // 预估次数
    let finder = get(&content, "Find")?.as_dispatch()?; // Find
    set(&finder, "Text", Variant::Str(find.clone()))?; // 查找文本
    set(&finder, "MatchCase", Variant::Bool(match_case))?; // 大小写
    set(&finder, "MatchWholeWord", Variant::Bool(whole_word))?; // 全字
    set(&finder, "MatchWildcards", Variant::Bool(false))?; // 关闭通配符
    set(&finder, "Forward", Variant::Bool(true))?; // 前向
    set(&finder, "Wrap", Variant::I4(WD_FIND_CONTINUE))?; // 环绕
    if let Ok(repl) = get(&finder, "Replacement").and_then(|v| v.as_dispatch()) { let _ = set(&repl, "Text", Variant::Str(replacement.clone())); } // Replacement.Text
    let execute = [ // Word Find.Execute 15 个位置参数
        Variant::Str(find), Variant::Bool(match_case), Variant::Bool(whole_word), Variant::Bool(false), // FindText/MatchCase/MatchWholeWord/MatchWildcards
        Variant::Bool(false), Variant::Bool(false), Variant::Bool(true), Variant::I4(WD_FIND_CONTINUE), // SoundsLike/AllWordForms/Forward/Wrap
        Variant::Bool(false), Variant::Str(replacement), Variant::I4(if all { WD_REPLACE_ALL } else { WD_REPLACE_ONE }), // Format/ReplaceWith/Replace
        Variant::Bool(false), Variant::Bool(false), Variant::Bool(false), Variant::Bool(false), // Kashida/Diacritics/AlefHamza/Control
    ]; // 参数表
    let hit = call(&finder, "Execute", &execute)?.as_bool(); // 执行
    let replaced = if hit { if all { count.max(1) } else { 1 } } else { 0 }; // 次数
    Ok(json!({"count": replaced, "replacements": []})) // 返回
}
fn goto(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 跳转
    let sel = selection(ctx)?; // Selection
    let what = str_arg(args, "what"); // 目标
    let which = match str_arg(args, "which").as_str() { "last" => WD_GO_TO_LAST, "next" => WD_GO_TO_NEXT, "previous" => WD_GO_TO_PREVIOUS, _ => WD_GO_TO_FIRST }; // 序号
    let which_value = if what == "end" { WD_STORY } else { which }; // 文末
    let name = str_arg(args, "name"); // 书签
    call(&sel, "GoTo", &[Variant::I4(which_value), Variant::I4(1), Variant::I4(1), Variant::Str(name)])?; // 跳转
    let start = get(&sel, "Start").map(|v| v.as_i32()).unwrap_or(0); // 光标位置
    let page = call(&sel, "Information", &[Variant::I4(WD_ACTIVE_END_PAGE_NUMBER)]).map(|v| v.as_i32()).unwrap_or(1); // 页码
    Ok(json!({"page": page, "paragraphIndex": paragraph_index_at(&ctx.doc, start)})) // 返回
}
fn goto_paragraph(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 段落跳转
    let index = i32_arg(args, "index", 1).max(1); // 序号
    let paras = get(&ctx.doc, "Paragraphs")?.as_dispatch()?; // 集合
    let para = call(&paras, "Item", &[Variant::I4(index)])?.as_dispatch()?; // 段落
    let range = get(&para, "Range")?.as_dispatch()?; // Range
    call(&range, "Select", &[])?; // 选中
    let page = call(&range, "Information", &[Variant::I4(WD_ACTIVE_END_PAGE_NUMBER)]).map(|v| v.as_i32()).unwrap_or(1); // 页码
    Ok(json!({"paragraphIndex": index, "page": page})) // 返回
}
fn select_all(ctx: &crate::com::document::DocCtx) -> ComResult<Value> { // 全选
    let content = get(&ctx.doc, "Content")?.as_dispatch()?; // 正文
    call(&content, "Select", &[])?; // 选中
    let chars = get(&content, "End")?.as_i32(); // 字符数
    Ok(json!({"selected": chars})) // 返回
}
fn select_text(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 选文本
    let start = i32_arg(args, "start", 0); // 起点
    let length = i32_arg(args, "length", 1); // 长度
    let content = get(&ctx.doc, "Content")?.as_dispatch()?; // 正文
    let total = get(&content, "End").map(|v| v.as_i32()).unwrap_or(0); // 总位置
    if start < 0 || length < 0 || start.saturating_add(length) > total { return Err(ToolError::new("INVALID_ARGUMENT", format!("start={start}/length={length} 超出正文 {total}"))); } // 越界
    let range = crate::com::dispatch::range(&ctx.doc, start, start + length)?; // Range
    call(&range, "Select", &[])?; // 选中
    let text = get(&range, "Text")?.as_string(); // 文本
    Ok(json!({"start": start, "end": start + length, "textPreview": text.chars().take(80).collect::<String>()})) // 返回
}
fn select_current(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 选当前词/段
    let sel = selection(ctx)?; // Selection
    let scope = str_arg(args, "scope"); // 范围
    if scope == "paragraph" { // 段落
        let paragraphs = get(&sel, "Paragraphs")?.as_dispatch()?; // 集合
        let first = call(&paragraphs, "Item", &[Variant::I4(1)])?.as_dispatch()?; // 第一段
        let range = get(&first, "Range")?.as_dispatch()?; // Range
        call(&range, "Select", &[])?; // 选中
    } else { // 词
        let words = get(&sel, "Words")?.as_dispatch()?; // 词集
        let first = call(&words, "Item", &[Variant::I4(1)])?.as_dispatch()?; // 第一个词
        call(&first, "Select", &[])?; // 选中
    }
    let start = get(&sel, "Start")?.as_i32(); // 起点
    let end = get(&sel, "End")?.as_i32(); // 终点
    Ok(json!({"start": start, "end": end, "textPreview": ""})) // 返回
}
fn select_at(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 定位选
    let loc = Locator::from_json(arg(args, "locator")); // 定位器
    let found = locate_range(&ctx.doc, &loc)?; // 定位
    crate::log_info!("edit", "select_at by={} match={} found={} start={} end={}", loc.by, loc.matched, found.found, found.start, found.end); // 诊断
    if !found.found { return Err(ToolError::new("INVALID_ARGUMENT", format!("定位失败 by={} match={} mode={}", loc.by, loc.matched, loc.match_mode))); } // 失败
    let range = crate::com::dispatch::range(&ctx.doc, found.start, found.end)?; // Range
    call(&range, "Select", &[])?; // 选中
    Ok(json!({"paragraphIndex": found.paragraph_index, "start": found.start, "end": found.end})) // 返回
}
pub fn edit(args: &Map<String, Value>) -> Value { // word_edit 5 action
    let action = str_arg(args, "action"); // 动作
    let path = str_arg(args, "path"); // 路径
    let paths = Paths::load(); // 策略
    let ctx = match active_document(&path, &paths) { Ok(c) => c, Err(e) => return err_json(&e) }; // 取文档
    if ctx.read_only() { return err_json(&ToolError::new("READ_ONLY", "文档为只读，无法编辑").recoverable("取消只读或另存为可写文档后重试")); } // 只读拦截
    let result: ComResult<Value> = match action.as_str() { // 分派
        "delete" => delete(&ctx, args), // 删除
        "backspace" => backspace(&ctx, args), // 退格
        "clipboard" => clipboard(&ctx, args), // 剪贴板
        "undo" => undo_redo(&ctx, args, true), // 撤销
        "redo" => undo_redo(&ctx, args, false), // 重做
        other => Err(ToolError::new("INVALID_ARGUMENT", format!("未知 edit action：{other}"))), // 未知
    };
    match result { Ok(v) => ok_json(v), Err(e) => err_json(&e) } // 返回
}
fn delete(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 删除
    let sel = selection(ctx)?; // Selection
    let unit = str_arg(args, "unit"); // 单位
    match unit.as_str() { // 分派
        "char" => { call(&sel, "Delete", &[])?; }, // 字符（当前选区）
        "word" => { let words = get(&sel, "Words")?.as_dispatch()?; let first = call(&words, "Item", &[Variant::I4(1)])?.as_dispatch()?; call(&first, "Delete", &[])?; }, // 词
        "paragraph" => { let paras = get(&sel, "Paragraphs")?.as_dispatch()?; let first = call(&paras, "Item", &[Variant::I4(1)])?.as_dispatch()?; let range = get(&first, "Range")?.as_dispatch()?; call(&range, "Delete", &[])?; }, // 段（Paragraph 无 Delete，改用 Range.Delete）
        "range" => { // 区间
            let range = arg(args, "range").and_then(|v| v.as_object()).ok_or_else(|| ToolError::new("INVALID_ARGUMENT", "unit=range 时必须提供 range 对象"))?; // 区间对象
            let start = range.get("start").and_then(|v| v.as_i64()).unwrap_or(0) as i32; // 起点
            let end = range.get("end").and_then(|v| v.as_i64()).unwrap_or(0) as i32; // 终点
            let r = crate::com::dispatch::range(&ctx.doc, start, end)?; // Range
            call(&r, "Delete", &[])?; // 删除
        }
        other => return Err(ToolError::new("INVALID_ARGUMENT", format!("unit 无效：{other}，可选 char/word/paragraph/range"))), // 白名单校验
    }
    Ok(json!({"deleted": true})) // 返回
}
fn backspace(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 退格
    let sel = selection(ctx)?; // Selection
    let count = i32_arg(args, "count", 1).clamp(1, 1000); // 次数
    for _ in 0..count { call(&sel, "TypeBackspace", &[])?; } // 逐次
    Ok(json!({"count": count})) // 返回
}
fn clipboard(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>) -> ComResult<Value> { // 剪贴板
    let sel = selection(ctx)?; // Selection
    let op = str_arg(args, "op"); // 操作
    let method = match op.as_str() { "copy" => "Copy", "cut" => "Cut", "paste" => "Paste", _ => return Err(ToolError::new("INVALID_ARGUMENT", "clipboard op 无效")) }; // 映射
    let mut last = None; // 最后错误
    for _ in 0..3 { // 剪贴板偶发占用，重试
        match call(&sel, method, &[]) { // 执行
            Ok(_) => { last = None; break; } // 成功
            Err(err) => { // 失败
                last = Some(err); // 记录
                if op == "paste" { // 粘贴回退
                    match call(&sel, "PasteAndFormat", &[Variant::I4(16)]) { // 保留原格式
                        Ok(_) => { last = None; break; } // 回退成功
                        Err(err) => last = Some(err), // 回退失败
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(120)); // 退避
            }
        }
    }
    if let Some(err) = last { return Err(err); } // 三次仍失败
    Ok(json!({"op": op})) // 返回
}
fn undo_redo(ctx: &crate::com::document::DocCtx, args: &Map<String, Value>, undo: bool) -> ComResult<Value> { // 撤销重做
    let app = get(&ctx.doc, "Application")?.as_dispatch()?; // Application
    let count = i32_arg(args, "count", 1).clamp(1, 100); // 次数
    let method = if undo { "Undo" } else { "Redo" }; // 方法
    for _ in 0..count { if call(&app, method, &[]).is_err() { let _ = get(&app, method); } } // 逐次（方法/属性回退）
    Ok(json!({"count": count})) // 返回
}
