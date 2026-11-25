use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::dispatch::{call, get, set, Variant}; // COM 工具
use crate::com::document::active_document; // 文档上下文
use crate::word::backup::backup_before_save; // 备份
use crate::word::locator::{locate_range, Locator}; // 定位
use crate::word::markdown::{render, RenderReport}; // Markdown
use crate::word::paths::Paths; // 路径
use crate::word::report::{err_json, ok_json, Report, ToolError}; // 信封
fn arg<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a Value> { args.get(key) } // 取参数
fn str_arg(args: &Map<String, Value>, key: &str) -> String { arg(args, key).and_then(|v| v.as_str()).unwrap_or("").to_string() } // 字符串
pub fn run(args: &Map<String, Value>) -> Value { // word_write
    let path = str_arg(args, "path"); // 路径
    if path.is_empty() { return err_json(&ToolError::new("INVALID_ARGUMENT", "word_write 必须传 path")); } // 校验
    let markdown = str_arg(args, "markdown"); // Markdown
    let file = str_arg(args, "file"); // 插入文件
    if markdown.is_empty() && file.is_empty() { return err_json(&ToolError::new("INVALID_ARGUMENT", "markdown 与 file 至少要有一个")); } // 二选一
    if markdown.chars().count() > 100000 { return err_json(&ToolError::new("INVALID_ARGUMENT", "markdown 超过 100000 字符")); } // 大小限制
    let mode = str_arg(args, "mode"); // insert/replace_selection
    let save = arg(args, "save").and_then(|v| v.as_bool()).unwrap_or(false); // 写后保存
    let loc = Locator::from_json(arg(args, "position")); // 位置
    let paths = Paths::load(); // 策略
    let ctx = match active_document(&path, &paths) { Ok(c) => c, Err(e) => return err_json(&e) }; // 取文档
    if let Err(err) = crate::com::document::ensure_writable(&ctx.app) { return err_json(&err); } // 对话框拦截
    if ctx.read_only() { return err_json(&ToolError::new("READ_ONLY", "文档为只读，无法写入").recoverable("取消只读或另存为可写文档后重试")); } // 只读拦截
    if !crate::word::instances::same_path(&ctx.full_name(), &ctx.path.to_string_lossy()) { // 活动文档校验
        return err_json(&ToolError::new("DOC_CHANGED", "活动文档与目标不一致").recoverable("重新打开目标文档")); // 切换
    }
    let mut report = Report::default(); // 报告
    let found = match locate_range(&ctx.doc, &loc) { Ok(r) => r, Err(e) => return err_json(&e) }; // 定位
    if !found.found { return err_json(&ToolError::new("INVALID_ARGUMENT", format!("定位失败 by={} match={} mode={}", loc.by, loc.matched, loc.match_mode))); } // 失败
    let result = if !file.is_empty() { // 文件插入
        let fp = match paths.resolve(&file) { Ok(p) => p, Err(e) => return err_json(&e) }; // 校验
        let target = if loc.by == "cursor" || loc.by == "end" { // 光标/文末沿用 Selection
            let app = match get(&ctx.doc, "Application").and_then(|v| v.as_dispatch()) { Ok(a) => a, Err(e) => return err_json(&e) }; // Application
            let sel = match get(&app, "Selection").and_then(|v| v.as_dispatch()) { Ok(s) => s, Err(e) => return err_json(&e) }; // Selection
            if loc.by == "end" { call(&sel, "EndKey", &[Variant::I4(6)]).ok(); } // 移到文末
            sel
        } else { // 按 position 折叠到目标位置
            let content = match get(&ctx.doc, "Content").and_then(|v| v.as_dispatch()) { Ok(c) => c, Err(e) => return err_json(&e) }; // 正文
            let start = found.start.max(1); // Word 位置从 1 起
            if let Err(e) = set(&content, "Start", Variant::I4(start)) { return err_json(&e); } // 起点
            if let Err(e) = set(&content, "End", Variant::I4(start)) { return err_json(&e); } // 折叠
            content
        };
        match call(&target, "InsertFile", &[Variant::Str(fp.to_string_lossy().to_string())]) { Ok(_) => Ok(RenderReport { blocks: 1, chars: 0 }), Err(err) => Err(err) } // 插入
    } else { // Markdown 渲染
        if mode == "replace_selection" { // 替换选区
            let app = get(&ctx.doc, "Application").and_then(|v| v.as_dispatch()).ok(); // Application
            if let Some(app) = app { if let Ok(sel) = get(&app, "Selection").and_then(|v| v.as_dispatch()) { let _ = call(&sel, "Delete", &[]); } } // 删除选区
        }
        render(&ctx.doc, &markdown, &loc, &mut report) // 渲染
    };
    match result { // 处理
        Ok(stats) => { // 成功
            let end = get(&ctx.doc, "Content").and_then(|value| value.as_dispatch()).and_then(|content| get(&content, "End")).map(|value| value.as_i32()).unwrap_or(found.end); // 写入后文末位置
            let mut data = json!({"blocks": stats.blocks, "chars": stats.chars, "paragraphStart": found.start, "paragraphEnd": end, "position": loc.by}); // 数据
            if save { // 保存
                let backup = match backup_before_save(&ctx.path) { Ok(value) => value, Err(error) => return err_json(&error) }; // 备份失败中止
                match ctx.save(backup.as_ref()) { Ok(_) => { data["saved"] = json!(true); }, Err(e) => return err_json(&e) } // 保存结果
            }
            report.ok_json(data) // 返回
        }
        Err(err) => err_json(&err), // 失败
    }
}
