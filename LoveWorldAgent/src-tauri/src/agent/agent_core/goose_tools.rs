use std::collections::HashMap; // goose 工具适配：现有 ToolDef 转 rmcp Tool，并按 ToolProvider 分发执行
use std::marker::PhantomData; // 泛型会话类型占位，不实际持有
use std::sync::Arc; // 工具提供者跨线程共享
use async_trait::async_trait; // 异步 trait 宏
use goose_agent::operation::Emitter; // 工具事件发射器
use goose_agent::tool::{ToolOperation, ToolProvider}; // 工具操作与提供者 trait
use rmcp::model::{CallToolRequestParams, CallToolResult, ContentBlock, ErrorData, ImageContent, JsonObject, Tool}; // rmcp 工具调用类型
use serde_json::{json, Value}; // JSON 构造与动态值
use crate::agent::tools::ToolDef; // 内部工具定义
#[derive(Clone)] // 路由可克隆
pub struct ToolRoute { // 工具路由：审批门按分组与命令字段判定是否需要用户批准
    pub group: String, // 工具分组（browser/mcp/command 等）
    pub command_key: String, // 工具参数中命令字段名
}
pub struct GooseToolProvider<S> { // 把内部工具定义包装成 rmcp 工具提供者
    defs: Vec<ToolDef>, // 工具定义列表
    _session: PhantomData<fn() -> S>, // 会话类型占位
}
fn schema_of(def: &ToolDef) -> Arc<JsonObject> { // 从工具定义生成 JSON Schema
    Arc::new(serde_json::from_value::<JsonObject>(def.parameters.clone()).unwrap_or_default()) // Schema 解析失败按空对象处理
}
fn tool_of(def: &ToolDef) -> Tool { // 将内部 ToolDef 转成 rmcp Tool
    Tool::new(def.name.clone(), def.description.clone(), schema_of(def)) // 填入名称、描述与参数 Schema
}
pub fn routes_of(defs: &[ToolDef]) -> HashMap<String, ToolRoute> { // 建立工具名到路由的映射
    defs.iter().map(|def| (def.name.clone(), ToolRoute { group: def.group.clone(), command_key: def.command_key.clone() })).collect() // 路由携带分组与命令字段供审批判定
}
pub fn build_goose_tools<S>(defs: Vec<ToolDef>) -> Arc<dyn ToolProvider<S>> // 构造 goose 工具提供者
where // 泛型约束起始
    S: Send + Sync + 'static, // 会话类型需跨线程且静态
{ // 函数体起始
    Arc::new(GooseToolProvider { defs, _session: PhantomData }) // 包装工具定义列表
}
#[async_trait] // 为泛型实现异步 trait
impl<S> ToolProvider<S> for GooseToolProvider<S> // 实现工具提供者
where // 泛型约束起始
    S: Send + Sync + 'static, // 会话类型需跨线程且静态
{ // 实现体起始
    async fn tools(&self, _session: &S) -> anyhow::Result<Vec<Tool>> { // 返回全部工具的 rmcp 描述
        Ok(self.defs.iter().map(tool_of).collect()) // 逐工具转换
    }
    async fn call(&self, _session: &S, _request_id: &str, call: CallToolRequestParams, _emit: &Emitter) -> Result<CallToolResult, ErrorData> { // 按调用名分发执行工具
        let Some(def) = self.defs.iter().find(|def| def.name == call.name.as_ref()) else { // 查找目标工具定义
            return Err(ErrorData::invalid_params(format!("未知工具 {}", call.name), None)); // 找不到工具返回非法参数错误
        };
        let arguments = call.arguments.map(Value::Object).unwrap_or_else(|| json!({})); // 取参数对象，缺省空对象
        let text = def.invoke(&arguments.to_string()).await; // await 调用工具处理器执行
        let mut blocks = vec![ContentBlock::text(text)]; // 工具正文仍走 text 内容块
        for (mime, data) in crate::agent::tools_details::common::take_tool_images(&def.name) { blocks.push(ContentBlock::Image(ImageContent::new(data, mime))); } // 截图转为 image 内容块
        let mut result = CallToolResult::success(blocks); // 组装结果
        let meta = crate::agent::tools_details::common::take_tool_meta(&def.name); // 取走旁路元数据，供前端 diff 等卡片渲染
        if meta.as_object().map(|item| !item.is_empty()).unwrap_or(false) { result.structured_content = Some(meta); } // 非空才附加，避免污染普通工具结果
        Ok(result) // 返回带正文与可选元数据的结果
    }
}
pub fn build_goose_tool_operation<S>(defs: Vec<ToolDef>) -> ToolOperation<S> // 构造不带审批的工具操作
where // 泛型约束起始
    S: Send + Sync + 'static, // 会话类型需跨线程且静态
{ // 函数体起始
    ToolOperation::new().with_provider(build_goose_tools(defs)) // 注入无审批工具提供者
}
pub fn build_goose_tool_operation_with_approval<S>(defs: Vec<ToolDef>, approval: Arc<dyn crate::agent::agent_core::goose_approval::ToolApproval>) -> ToolOperation<S> // 构造带审批门的工具操作（子智能体 Ask 走阻塞式审批桥）
where // 泛型约束起始
    S: Send + Sync + 'static, // 会话类型需跨线程且静态
{ // 函数体起始
    let inner = build_goose_tools(defs.clone()); // 先构建内部工具提供者
    ToolOperation::new().with_provider(crate::agent::agent_core::goose_approval::build_guarded_tools_with_bridge(inner, &defs, approval)) // 再用带审批桥的门包装后交给 ToolOperation
}
pub fn build_mcp_tool_defs(runtime: &crate::agent::mcp::McpRuntime) -> Vec<ToolDef> { // 从 MCP 运行时生成工具定义
    crate::agent::mcp::tool_defs(runtime) // 委托 mcp 模块转换
}
pub async fn build_goose_mcp_provider<S>() -> (Arc<dyn ToolProvider<S>>, std::collections::BTreeMap<String, Vec<String>>) // 异步构造 MCP 工具提供者与目录
where // 泛型约束起始
    S: Send + Sync + 'static, // 会话类型需跨线程且静态
{ // 函数体起始
    let runtime = crate::agent::mcp::connect_all().await; // await 连接全部 MCP 服务
    let catalog = runtime.catalog.clone(); // 复制工具目录用于系统提示词
    (build_goose_tools(build_mcp_tool_defs(&runtime)), catalog) // 返回提供者与目录
}
