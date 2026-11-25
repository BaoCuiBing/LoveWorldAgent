use std::future::Future; // 工具门面：统一 ToolDef（名称/描述/JSON Schema/审批组/执行器）、调用日志与审批判定
use std::pin::Pin; // 固定异步 Future
use std::sync::Arc; // 处理器跨线程共享
use std::time::Instant; // 调用耗时计时
use serde_json::{json, Value}; // JSON 构造与动态值
use crate::agent::permission; // 审批规则判定
use crate::utils::logger; // 调用日志
use crate::agent::tools_details::common::{squash, truncate_chars, TOOL_LOG_LIMIT}; // 日志文本压缩与截断
pub type ToolFuture = Pin<Box<dyn Future<Output = String> + Send>>; // 工具异步返回类型
pub type ToolHandler = Arc<dyn Fn(String) -> ToolFuture + Send + Sync>; // 工具处理器类型
#[derive(Clone)] // 工具定义可克隆
pub struct ToolDef { // 统一工具定义：名称/描述/Schema/审批组/执行器
    pub name: String, // 工具名
    pub description: String, // 工具描述
    pub parameters: Value, // 参数 JSON Schema
    pub group: String, // 审批分组
    pub command_key: String, // 命令字段名
    pub handler: ToolHandler, // 执行器
}
fn schema_of(kind: &str) -> Value { // 把字段类型映射为 JSON Schema 片段
    match kind { // 按类型字符串分派
        "string" => json!({"type": "string"}), // 字符串
        "integer" => json!({"type": "integer"}), // 整数
        "number" => json!({"type": "number"}), // 数字
        "boolean" => json!({"type": "boolean"}), // 布尔
        "string_array" => json!({"type": "array", "items": {"type": "string"}}), // 字符串数组
        "integer_array" => json!({"type": "array", "items": {"type": "integer"}}), // 整数数组
        "number_array" => json!({"type": "array", "items": {"type": "number"}}), // 数字数组
        _ => json!({"type": "string"}), // 未知类型按字符串处理
    }
}
pub fn params(fields: &[(&str, &str, &str)]) -> Value { // 由字段名/类型/描述构造参数 Schema
    let mut properties = serde_json::Map::new(); // 属性表
    let mut required: Vec<Value> = Vec::new(); // 必填字段列表
    for (name, kind, description) in fields { // 遍历字段定义
        let mut item = schema_of(kind); // 生成类型片段
        if let Some(map) = item.as_object_mut() { map.insert("description".to_string(), json!(description)); } // 注入字段描述
        properties.insert((*name).to_string(), item); // 登记属性
        if !description.contains("可选") { required.push(json!(name)); } // 描述未标"可选"即必填
    }
    json!({"type": "object", "properties": properties, "required": required, "additionalProperties": false}) // 组装对象 Schema，禁止额外字段
}
pub fn blocking<F>(func: F) -> ToolHandler where F: Fn(String) -> String + Send + Sync + 'static { // 把同步函数包装成异步工具处理器
    let func = Arc::new(func); // 共享同步函数
    Arc::new(move |args: String| { // 构造处理器闭包
        let func = func.clone(); // 克隆函数引用
        Box::pin(async move { tokio::task::spawn_blocking(move || func(args)).await.unwrap_or_else(|err| format!("工具执行失败：{err}")) }) // 丢到阻塞线程池执行
    })
}
pub fn blocking_result<F>(func: F) -> ToolHandler where F: Fn(String) -> Result<String, String> + Send + Sync + 'static { // 包装返回 Result 的同步函数
    blocking(move |args: String| func(args).unwrap_or_else(|err| format!("工具执行失败：{err}"))) // 出错转为错误文案
}
impl ToolDef { // 工具定义构造与调用
    pub fn new(name: &str, description: &str, parameters: Value, handler: ToolHandler) -> Self { // 构造工具定义
        Self { name: name.to_string(), description: description.to_string(), parameters, group: String::new(), command_key: String::new(), handler } // 分组与命令字段默认为空
    }
    pub fn with_group(mut self, group: &str) -> Self { // 设置审批分组
        self.group = group.to_string(); // 写入分组
        self // 返回自身支持链式调用
    }
    pub fn with_command(mut self, key: &str) -> Self { // 设置命令字段名
        self.command_key = key.to_string(); // 写入命令字段
        self // 返回自身支持链式调用
    }
    pub fn schema(&self) -> Value { // 生成 function calling 格式 Schema
        json!({"type": "function", "function": {"name": self.name, "description": self.description, "parameters": self.parameters}}) // 包装为 OpenAI function 结构
    }
    pub async fn invoke(&self, arguments: &str) -> String { // 执行工具并记录调用日志
        let begin = Instant::now(); // 记录起始时间
        logger::debug("tools", &format!("-> 工具 {}({})", self.name, truncate_chars(&squash(arguments), TOOL_LOG_LIMIT))); // 记录入参摘要
        let result = (self.handler)(arguments.to_string()).await; // await 执行处理器
        logger::debug("tools", &format!("<- 工具 {} = {} ({:.1}ms)", self.name, truncate_chars(&squash(&result), TOOL_LOG_LIMIT), begin.elapsed().as_secs_f64() * 1000.0)); // 记录结果与耗时
        result // 返回工具输出
    }
}
pub fn command_of(arguments: &Value, key: &str) -> String { // 从参数中提取命令文本
    arguments.get(key).map(|value| match value { Value::String(text) => text.clone(), other => other.to_string() }).unwrap_or_default() // 字符串直接取，其他转 JSON 文本
}
pub fn approval_reason(tool: &ToolDef, arguments: &Value) -> Option<String> { // 判定工具是否需要审批并给出原因
    if tool.name == "ask_user" { return None; } // 提问工具无需审批
    if let Some(reason) = permission::tool_blocked(&tool.group, &tool.name) { return Some(reason); } // 组级硬拦截（电脑控制组关闭）
    if tool.group == "browser" || tool.group == "mcp" || tool.group == "computer" || tool.group == "word" || tool.group == "ppt" || tool.group == "excel" || tool.group == "mobile" { return if permission::tool_needs_approval(&tool.group, &tool.name) { Some(permission::tool_reason(&tool.group, &tool.name)) } else { None }; } // 浏览器/MCP/电脑/Office/移动端组按配置审批
    if tool.command_key.is_empty() { return None; } // 无命令字段的工具直接放行
    let command = command_of(arguments, &tool.command_key); // 提取命令文本
    if command.trim().is_empty() { return None; } // 命令为空时放行
    if permission::needs_approval(&command) { Some(permission::reason(&command)) } else { None } // 命中敏感命令规则时给出原因
}
