use serde_json::{json, Value}; // 待办工具：todo_write 每次提交全量清单，规范化后写回模块级状态并记录旁路元数据
use crate::agent::tools::{params, blocking, ToolDef}; // 工具定义与同步包装
use crate::agent::tools_details::common::{remember_tool_meta, set_todos, TODO_LIMIT, TODO_STATUSES}; // 状态写入与常量
pub fn build_todo_tool(enabled: bool) -> Vec<ToolDef> { // 构造待办工具，禁用时返回空列表
    if !enabled { return Vec::new(); } // 会话关闭待办则返回空
    let tool = ToolDef::new("todo_write", "维护多步任务的待办清单；items 为 JSON 数组字符串，元素形如 {\"content\":\"事项\",\"status\":\"pending|in_progress|completed\"}；每次提交全量清单，完成一步立即更新。单步任务不要使用", params(&[("items", "string", "JSON 数组字符串")]), blocking(move |args: String| { // 待办工具定义与处理器
        let value: Value = serde_json::from_str(&args).unwrap_or_else(|_| json!({})); // 解析参数，非法按空对象
        let raw = value.get("items").cloned().unwrap_or_else(|| json!("[]")); // 取 items 字段
        let parsed = match raw { // items 可能是 JSON 字符串或数组
            Value::String(text) => serde_json::from_str::<Value>(&text).unwrap_or_else(|_| Value::Array(Vec::new())), // 字符串则再解析
            other => other, // 数组直接用
        };
        let Value::Array(items) = parsed else { // 非数组视为格式错误
            remember_tool_meta("todo_write", json!({"ok": false, "error": "待办格式错误"})); // 写入错误元数据
            return "待办格式错误：需要 JSON 数组".to_string(); // 返回错误文案
        };
        let mut rows: Vec<Value> = Vec::new(); // 规范化待办列表
        for item in items { // 逐条待办
            if rows.len() >= TODO_LIMIT { break; } // 达到上限停止
            let Some(map) = item.as_object() else { continue; }; // 非对象跳过
            let content = map.get("content").and_then(Value::as_str).unwrap_or_default().trim().to_string(); // 事项内容
            if content.is_empty() { continue; } // 空事项跳过
            let status = map.get("status").and_then(Value::as_str).unwrap_or_default().trim().to_lowercase(); // 状态字段
            let status = if TODO_STATUSES.contains(&status.as_str()) { status } else { "pending".to_string() }; // 非法状态回退 pending
            rows.push(json!({"content": content, "status": status})); // 写入规范化待办
        }
        let saved = set_todos(rows); // 写回模块级状态
        let completed = saved.iter().filter(|item| item.get("status").and_then(Value::as_str) == Some("completed")).count(); // 统计已完成数
        remember_tool_meta("todo_write", json!({"ok": true, "items": saved, "total": saved.len(), "completed": completed})); // 写入成功元数据
        serde_json::to_string(&saved).unwrap_or_else(|_| "[]".to_string()) // 返回清单 JSON
    }));
    vec![tool] // 返回工具列表
}
