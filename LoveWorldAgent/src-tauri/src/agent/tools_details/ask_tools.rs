use serde_json::{json, Value}; // 主动提问工具：ask_user 每次调用都中断等待用户回答，批准续流后按题目指纹取回答案
use crate::agent::tools::{params, blocking, ToolDef}; // 工具定义与同步包装
use crate::agent::tools_details::common::{remember_tool_meta, take_answer}; // 元数据写入与答案取回
pub fn build_ask_tool(enabled: bool) -> Vec<ToolDef> { // 构造提问工具，禁用时返回空列表
    if !enabled { return Vec::new(); } // 会话关闭提问则返回空
    let tool = ToolDef::new("ask_user", "仅在任务目标、范围或交付形式不明确时向用户提问。questions 为 JSON 数组字符串，元素结构：{\"question\":\"问题\",\"options\":[\"选项A\",\"选项B\"],\"mode\":\"single|multiple\",\"customNote\":true}；options 为空时前端显示自由文本输入；一次可提多个问题，全部问题必答，用户回答后继续", params(&[("questions", "string", "JSON 数组字符串，元素含 question/options/mode/customNote")]), blocking(move |args: String| { // 提问工具定义与处理器
        let value: Value = serde_json::from_str(&args).unwrap_or_else(|_| json!({})); // 解析参数，非法按空对象
        let questions = value.get("questions").and_then(Value::as_str).unwrap_or("[]").to_string(); // 取问题 JSON 文本
        let answer = take_answer(&questions); // 按题目指纹取回用户答案
        let mut rows: Vec<Value> = Vec::new(); // 规范化问题列表
        if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&questions) { // 解析问题数组
            for item in items { // 逐题
                if let Some(map) = item.as_object() { // 只处理对象项
                    let options = map.get("options").and_then(Value::as_array).cloned().unwrap_or_default(); // 只保留选项数组
                    let mode = if map.get("mode").and_then(Value::as_str) == Some("multiple") { "multiple" } else { "single" }; // 单选或多选
                    rows.push(json!({"question": map.get("question").and_then(Value::as_str).unwrap_or_default(), "options": options, "mode": mode, "customNote": map.get("customNote").and_then(Value::as_bool).unwrap_or(false)})); // 仅保留四个契约字段
                }
            }
        }
        remember_tool_meta("ask_user", json!({"questions": rows, "answered": !answer.is_empty()})); // 写回问题与回答状态元数据
        if answer.is_empty() { "用户尚未回答，请等待。".to_string() } else { answer } // 未回答返回等待提示，否则返回答案
    }));
    vec![tool] // 返回工具列表
}