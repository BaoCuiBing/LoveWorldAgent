use serde_json::{json, Map, Value}; // JSON 构造
use crate::com::dispatch::{get_args, call, get, set, ComResult, Variant}; // COM 工具
use crate::com::document::active_document; // 文档上下文
use crate::word::locator::{locate_range, Locator}; // 定位
use crate::word::paths::Paths; // 路径
use crate::word::report::{err_json, ok_json, ToolError}; // 信封
fn arg<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a Value> { args.get(key) } // 取参数
fn str_arg(args: &Map<String, Value>, key: &str) -> String { arg(args, key).and_then(|v| v.as_str()).unwrap_or("").to_string() } // 字符串
pub fn variable(args: &Map<String, Value>) -> Value { // word_variable
    let path = str_arg(args, "path"); // 路径
    let vars = arg(args, "variables").and_then(|v| v.as_object()); // 变量集
    let Some(vars) = vars else { return err_json(&ToolError::new("INVALID_ARGUMENT", "variables 必须是对象")); }; // 校验
    if vars.is_empty() || vars.len() > 100 { return err_json(&ToolError::new("INVALID_ARGUMENT", "variables 需 1~100 项")); } // 数量
    let paths = Paths::load(); // 策略
    let ctx = match active_document(&path, &paths) { Ok(c) => c, Err(e) => return err_json(&e) }; // 取文档
    if let Err(err) = crate::com::document::ensure_writable(&ctx.app) { return err_json(&err); } // 对话框拦截
    let scope = str_arg(args, "scope"); // 范围
    let target = if scope == "selection" { // 选区
        let app = match get(&ctx.doc, "Application").and_then(|v| v.as_dispatch()) { Ok(a) => a, Err(e) => return err_json(&e) }; // Application
        match get(&app, "Selection").and_then(|v| v.as_dispatch()) { Ok(s) => s, Err(e) => return err_json(&e) } // Selection
    } else { ctx.doc.clone() }; // 全文
    let mut replaced = Vec::new(); // 结果
    let mut total = 0; // 总数
    for (key, value) in vars { // 逐变量
        let text = match value { Value::String(s) => s.clone(), other => other.to_string() }; // 值
        let patterns = [format!("{{{{{key}}}}}"), format!("{{{key}}}")]; // 两种占位
        let mut count = 0; // 计数
        for pattern in &patterns { // 逐模式
            match replace_all(&target, pattern, &text) { Ok(n) => count += n, Err(e) => return err_json(&e) } // 替换
        }
        replaced.push(json!({"key": key, "count": count})); // 收集
        total += count; // 累加
    }
    ok_json(json!({"replaced": replaced, "total": total})) // 返回
}
fn replace_all(target: &crate::com::dispatch::ComObj, pattern: &str, text: &str) -> ComResult<i32> { // 全文替换单模式
    let content = match get(target, "Content") { Ok(v) => v.as_dispatch().unwrap_or_else(|_| target.clone()), Err(_) => target.clone() }; // 内容
    let finder = get(&content, "Find").or_else(|_| get(target, "Find"))?.as_dispatch()?; // Find
    set(&finder, "Text", Variant::Str(pattern.to_string()))?; // 查找
    let repl = get(&finder, "Replacement")?.as_dispatch()?; // 替换对象
    set(&repl, "Text", Variant::Str(text.to_string()))?; // 替换文本
    set(&finder, "Wrap", Variant::I4(1))?; // 环绕
    set(&finder, "Format", Variant::Bool(false))?; // 不替换格式
    set(&finder, "Forward", Variant::Bool(true))?; // 前向
    set(&finder, "MatchWildcards", Variant::Bool(false))?; // 关闭通配符
    let mut count = 0; // 计数
    for _ in 0..10000 { // 上限防死循环
        let execute = [Variant::Str(pattern.to_string()), Variant::Bool(false), Variant::Bool(false), Variant::Bool(false), Variant::Bool(false), Variant::Bool(false), Variant::Bool(true), Variant::I4(1), Variant::Bool(false), Variant::Str(text.to_string()), Variant::I4(1)]; // Execute 参数（FindText/Replace=1）
        if !call(&finder, "Execute", &execute)?.as_bool() { break; } // 未命中结束
        count += 1; // 累加
    }
    Ok(count) // 返回
}
pub fn batch(args: &Map<String, Value>) -> Value { // word_batch
    let path = str_arg(args, "path"); // 路径
    let ops = arg(args, "ops").and_then(|v| v.as_array()); // 操作集
    let Some(ops) = ops else { return err_json(&ToolError::new("INVALID_ARGUMENT", "ops 必须是数组")); }; // 校验
    if ops.is_empty() || ops.len() > 50 { return err_json(&ToolError::new("INVALID_ARGUMENT", "ops 需 1~50 条")); } // 数量
    let paths = Paths::load(); // 策略
    let ctx = match active_document(&path, &paths) { Ok(c) => c, Err(e) => return err_json(&e) }; // 取文档
    let mut results = Vec::new(); // 结果
    let mut partial = false; // 部分失败
    for (index, op) in ops.iter().enumerate() { // 逐条
        let outcome = (|| -> ComResult<Value> { // 单条
            let action = op.get("action").and_then(|v| v.as_str()).unwrap_or(""); // 动作
            if action != "insert" { return Err(ToolError::new("INVALID_ARGUMENT", "batch 仅支持 insert")); } // 仅插入
            let text = op.get("text").and_then(|v| v.as_str()).unwrap_or(""); // 文本
            if text.chars().count() > 100000 { return Err(ToolError::new("INVALID_ARGUMENT", "text 超过 100000 字符")); } // 限制
            let loc = Locator::from_json(op.get("locator")); // 定位器
            let found = locate_range(&ctx.doc, &loc)?; // 定位
            if !found.found { return Err(ToolError::new("INVALID_ARGUMENT", "定位失败")); } // 失败
            let range = crate::com::dispatch::range(&ctx.doc, found.start, found.end)?; // Range
            set(&range, "Text", Variant::Str(text.to_string()))?; // 插入
            Ok(json!({"inserted": text.chars().count()})) // 返回
        })(); // 执行
        match outcome { // 收集
            Ok(value) => results.push(json!({"index": index, "ok": true, "data": value})), // 成功
            Err(err) => { partial = true; results.push(json!({"index": index, "ok": false, "error": err.to_json()})); } // 失败不中断
        }
    }
    ok_json(json!({"results": results, "partial": partial})) // 返回
}
