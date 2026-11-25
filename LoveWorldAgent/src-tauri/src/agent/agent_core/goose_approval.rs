use std::collections::HashMap; // goose 审批门：ToolApproval 语义（Allow/Deny/Ask）与 ToolProvider 包装
use std::sync::Arc; // 审批器与提供者跨线程共享
use std::time::Duration; // 子智能体审批等待超时
use async_trait::async_trait; // 异步 trait 宏
use goose_agent::operation::Emitter; // 工具事件发射器
use goose_agent::tool::ToolProvider; // 被包装的工具提供者 trait
use goose_provider_types::conversation::message::{Message, MessageContent}; // 审批请求消息构造
use goose_provider_types::permission::Permission; // 审批权限值
use once_cell::sync::Lazy; // 全局等待表惰性初始化
use parking_lot::Mutex; // 等待表互斥锁
use rmcp::model::{CallToolRequestParams, CallToolResult, ContentBlock, ErrorData, Tool}; // rmcp 工具调用类型
use serde_json::Value; // 工具参数动态值
use tokio::sync::oneshot; // 子智能体审批唤醒通道
use crate::agent::agent_core::goose_tools::{routes_of, ToolRoute}; // 工具路由表构造
use crate::agent::permission; // 权限判定与白名单读写
use crate::agent::tools::{command_of, ToolDef}; // 工具定义与命令字段提取
#[derive(Clone, Copy, PartialEq, Eq, Debug)] // 审批决策可复制比较
pub enum ApprovalDecision { Allow, Deny, Ask } // 放行/拒绝/需批准三种决策
pub struct ApprovalOutcome { // 审批结果：entry 为总是允许时要写回 custom 的条目
    pub decision: ApprovalDecision, // 决策类型
    pub entry: String, // 需记住的白名单条目
    pub reason: String, // 拒绝或需批准的原因
}
impl ApprovalOutcome { // 审批结果构造器
    pub fn allow(entry: &str) -> Self { // 构造允许结果
        Self { decision: ApprovalDecision::Allow, entry: entry.to_string(), reason: String::new() } // 允许无需原因
    }
    pub fn deny(entry: &str, reason: &str) -> Self { // 构造拒绝结果并带原因
        Self { decision: ApprovalDecision::Deny, entry: entry.to_string(), reason: reason.to_string() } // 拒绝携带原因
    }
    pub fn ask(entry: &str, reason: &str) -> Self { // 构造需用户批准结果
        Self { decision: ApprovalDecision::Ask, entry: entry.to_string(), reason: reason.to_string() } // 需批准携带原因
    }
}
pub trait ToolApproval: Send + Sync { // goose 侧无 ToolApproval 类型，这里按其审批语义提供可替换实现
    fn decide(&self, tool: &str, route: Option<&ToolRoute>, arguments: &Value) -> ApprovalOutcome; // 按工具与参数判定决策
    fn remember(&self, entry: &str); // 记住总是允许的条目
}
pub struct PermissionApproval; // 基于权限模块的默认审批器
impl ToolApproval for PermissionApproval { // 权限审批器实现
    fn decide(&self, tool: &str, route: Option<&ToolRoute>, arguments: &Value) -> ApprovalOutcome { // 按工具路由与参数判定审批
        let Some(route) = route else { return ApprovalOutcome::deny(tool, "未知工具：权限表中没有该工具，已拒绝执行。") }; // 无路由记录即视为未知工具并拒绝
        if tool == "ask_user" { return ApprovalOutcome::allow(tool); } // 提问工具直接放行
        if let Some(reason) = permission::tool_blocked(&route.group, tool) { return ApprovalOutcome::deny(tool, &reason); } // 组级硬拦截优先（电脑控制组关闭直接拒绝）
        if matches!(route.group.as_str(), "browser" | "mcp" | "computer" | "word" | "ppt" | "excel" | "mobile") { // 内置工具组与 MCP 按各自审批开关判定
            if permission::tool_needs_approval(&route.group, tool) { return ApprovalOutcome::ask(tool, &permission::tool_reason(&route.group, tool)); } // 命中需批准配置时返回 Ask
            return ApprovalOutcome::allow(tool); // 否则放行该组工具
        }
        if route.command_key.is_empty() { return ApprovalOutcome::allow(tool); } // 无命令字段的工具直接放行
        let command = command_of(arguments, &route.command_key); // 从参数中提取命令文本
        if command.trim().is_empty() { return ApprovalOutcome::allow(tool); } // 命令为空时放行
        if permission::needs_approval(&command) { return ApprovalOutcome::ask(&command, &permission::reason(&command)); } // 命中敏感命令规则时要求批准
        ApprovalOutcome::allow(&command) // 其余命令按白名单放行并记住条目
    }
    fn remember(&self, entry: &str) { // 写回总是允许的命令条目
        let _ = permission::append_custom(entry); // 追加到自定义白名单配置
    }
}
pub fn build_approval() -> Arc<dyn ToolApproval> { // 构造默认审批器
    Arc::new(PermissionApproval) // 返回权限审批器实例
}
pub const SUBAGENT_APPROVAL_TIMEOUT_SECS: u64 = 300; // 子智能体等待用户批准的时长
static SUBAGENT_WAITS: Lazy<Mutex<HashMap<String, oneshot::Sender<Permission>>>> = Lazy::new(|| Mutex::new(HashMap::new())); // 待审批 action id → 唤醒通道
#[derive(Clone)] // 桥接配置可克隆
pub struct ApprovalBridge { // 子智能体审批桥：Ask 时阻塞等待前端决策
    pub timeout: Duration, // 等待超时时长
}
pub fn build_approval_bridge() -> ApprovalBridge { // 构造默认审批桥
    ApprovalBridge { timeout: Duration::from_secs(SUBAGENT_APPROVAL_TIMEOUT_SECS) } // 默认 300 秒
}
pub fn resolve_subagent(action_id: &str, permission: Permission) -> bool { // 唤醒等待中的子智能体审批
    match SUBAGENT_WAITS.lock().remove(action_id) { Some(sender) => sender.send(permission).is_ok(), None => false } // 命中则发送决策
}
fn register_subagent_wait(action_id: &str) -> oneshot::Receiver<Permission> { // 注册等待通道
    let (tx, rx) = oneshot::channel(); // 创建一次性通道
    SUBAGENT_WAITS.lock().insert(action_id.to_string(), tx); // 登记到全局等待表
    rx // 返回接收端
}
fn drop_subagent_wait(action_id: &str) { // 清理超时/取消的等待项
    SUBAGENT_WAITS.lock().remove(action_id); // 移除等待通道
}
struct WaitGuard(String); // 等待项清理守卫：future 被丢弃时也要移除等待通道
impl Drop for WaitGuard { // 释放时清理
    fn drop(&mut self) { drop_subagent_wait(&self.0); } // 移除可能残留的等待项
}
async fn wait_permission(bridge: &ApprovalBridge, action_id: &str, tool: &str, arguments: &Value, reason: &str, emit: &Emitter) -> Permission { // 发出审批请求并阻塞等待决策
    let receiver = register_subagent_wait(action_id); // 先登记再发消息，避免前端响应早于注册
    let _guard = WaitGuard(action_id.to_string()); // 兜底清理等待项
    let arguments = arguments.as_object().cloned().unwrap_or_default(); // 取参数对象
    emit.message(Message::assistant().with_content(MessageContent::action_required(action_id.to_string(), tool.to_string(), arguments, Some(reason.to_string())))); // 经子智能体事件通道发出审批请求
    tokio::select! { // 超时、取消、用户决策三者先到为准
        result = tokio::time::timeout(bridge.timeout, receiver) => match result { // 等待用户决策
            Ok(Ok(permission)) => permission, // 收到决策
            _ => Permission::Cancel // 超时或通道关闭按拒绝处理
        },
        _ = emit.cancelled() => Permission::Cancel // 本轮取消按拒绝处理
    }
}
pub fn is_always_allowed(entry: &str) -> bool { // 判断条目是否已总是允许
    permission::whitelisted(entry, None) // 查询权限白名单
}
pub struct ApprovalGate<S> { // 审批门：包装内部提供者，调用前先判定
    inner: Arc<dyn ToolProvider<S>>, // 被包装的工具提供者
    approval: Arc<dyn ToolApproval>, // 审批器
    routes: HashMap<String, ToolRoute>, // 工具名到路由的映射
    bridge: Option<ApprovalBridge>, // 子智能体审批桥，主会话为空
}
impl<S> ApprovalGate<S> { // 审批门构造
    pub fn new(inner: Arc<dyn ToolProvider<S>>, approval: Arc<dyn ToolApproval>, routes: HashMap<String, ToolRoute>) -> Self { // 组装审批门
        Self { inner, approval, routes, bridge: None } // 保存内部提供者、审批器与路由，默认无桥接
    }
    pub fn with_bridge(mut self, bridge: Option<ApprovalBridge>) -> Self { // 启用子智能体审批桥
        self.bridge = bridge; // 保存桥接配置
        self // 返回自身便于链式构造
    }
}
pub fn build_guarded_tools<S>(inner: Arc<dyn ToolProvider<S>>, defs: &[ToolDef], approval: Arc<dyn ToolApproval>) -> Arc<dyn ToolProvider<S>> // 构造带审批门的工具提供者
where // 泛型约束起始
    S: Send + Sync + 'static, // 会话类型需跨线程且静态
{ // 函数体起始
    Arc::new(ApprovalGate::new(inner, approval, routes_of(defs))) // 用路由表构建审批门
}
pub fn build_guarded_tools_with_bridge<S>(inner: Arc<dyn ToolProvider<S>>, defs: &[ToolDef], approval: Arc<dyn ToolApproval>) -> Arc<dyn ToolProvider<S>> // 构造带阻塞式审批桥的工具提供者
where // 泛型约束起始
    S: Send + Sync + 'static, // 会话类型需跨线程且静态
{ // 函数体起始
    Arc::new(ApprovalGate::new(inner, approval, routes_of(defs)).with_bridge(Some(build_approval_bridge()))) // 启用子智能体审批桥
}
#[async_trait] // 为泛型实现异步 trait
impl<S> ToolProvider<S> for ApprovalGate<S> // 审批门实现工具提供者
where // 泛型约束起始
    S: Send + Sync + 'static, // 会话类型需跨线程且静态
{ // 实现体起始
    async fn tools(&self, session: &S) -> anyhow::Result<Vec<Tool>> { // 转发工具清单查询
        self.inner.tools(session).await // await 调用被包装者的 tools
    }
    async fn call(&self, session: &S, request_id: &str, call: CallToolRequestParams, emit: &Emitter) -> Result<CallToolResult, ErrorData> { // 工具调用前先做审批判定
        let arguments = call.arguments.clone().map(Value::Object).unwrap_or_else(|| Value::Object(Default::default())); // 取参数对象，缺省空对象
        let outcome = self.approval.decide(call.name.as_ref(), self.routes.get(call.name.as_ref()), &arguments); // 依据调用名查路由并判定
        match outcome.decision { // 按 Allow/Deny/Ask 三分支处理
            ApprovalDecision::Allow => self.inner.call(session, request_id, call, emit).await, // 允许时转发实际工具调用
            ApprovalDecision::Deny => Ok(CallToolResult::error(vec![ContentBlock::text(format!("权限策略拒绝：{}", outcome.reason))])), // 拒绝时写回错误结果
            ApprovalDecision::Ask => match &self.bridge { // 需批准时按是否启用桥接分派
                Some(bridge) => { // 子智能体走阻塞式审批桥
                    let tool = call.name.to_string(); // 工具名
                    let permission = wait_permission(bridge, request_id, &tool, &arguments, &outcome.reason, emit).await; // 发出审批请求并等待用户决策
                    match permission { // 按用户决策处理
                        Permission::AllowOnce | Permission::AlwaysAllow => self.inner.call(session, request_id, call, emit).await, // 批准时执行工具
                        _ => Ok(CallToolResult::error(vec![ContentBlock::text(format!("用户未批准执行：{}", outcome.reason))])), // 拒绝或超时按未批准返回
                    }
                }
                None => Ok(CallToolResult::error(vec![ContentBlock::text(format!("需要用户批准：{}", outcome.reason))])), // 无桥接时保持原行为
            },
        }
    }
}
