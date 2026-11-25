use std::collections::{HashMap, HashSet}; // 状态机装配：审批 → 提示词 → 工具 → 未注册工具 → 轮次上限 → 推理 → 出错退出，并驱动 SSE 事件
use std::sync::atomic::{AtomicBool, Ordering}; // 被动超窗兜底压缩单次防重入开关
use std::sync::Arc; // 步骤与提供者跨线程共享
use std::time::Duration; // 单次补全与子智能体超时
use anyhow::Result; // 状态机操作结果类型
use async_trait::async_trait; // 异步 trait 宏
use goose_agent::events::AgentEvent; // SSE 事件类型
use goose_agent::inference::InferenceRunner; // 推理步骤执行器
use goose_agent::machine::{EffectHandler, SessionLoader, StateMachine, Step}; // 状态机核心类型
use goose_agent::operation::{applied, assistant_turn_count, messages_since_kickoff, not_applicable, yielded, yielded_with, ConversationEffect, Emitter, Operation, OperationResult, RunStatus}; // 操作结果构造器与运行状态
use goose_provider_types::base::{MessageStream, Provider}; // Provider 包装与流类型
use goose_provider_types::conversation::{fix_conversation, merge_consecutive_messages_for_request, Conversation}; // 对话规范化与容器
use goose_provider_types::conversation::message::{Message, MessageContent, MessageErrorKind}; // 消息与内容块类型
use goose_provider_types::errors::ProviderError; // 模型错误类型
use goose_provider_types::model::ModelConfig as GooseModelConfig; // goose 模型配置类型
use goose_provider_types::permission::Permission; // 审批权限值
use goose_provider_types::thinking::ThinkingEffort; // 思考强度
use parking_lot::Mutex; // 内存运行态互斥锁
use rmcp::model::{CallToolResult, ContentBlock, ErrorData, JsonObject, Tool}; // rmcp 工具结果与定义类型
use serde_json::Value; // 动态 JSON 值
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender}; // SSE 事件无界通道
use tokio_util::sync::CancellationToken; // 取消令牌
use crate::agent::models::ModelConfig; // 模型配置
use crate::agent::settings::AgentSettings; // 智能体设置
use crate::agent::store_sqls::{messages as messages_store, sessions}; // 会话与消息数据库读写
use crate::agent::tools::ToolDef; // 工具定义
use crate::agent::tools_details::common::set_pending_answers; // 记住待回答问题
use crate::agent::agent_core::context; // 三层上下文压缩引擎
use crate::agent::agent_core::goose_approval::{self, ApprovalDecision, ToolApproval}; // 审批门
use crate::agent::agent_core::goose_session::{self, ChatEffect, GooseSession, SessionStore}; // 会话与效果适配
use crate::agent::agent_core::goose_tools::{self, ToolRoute}; // 工具提供者与路由
use crate::agent::agent_core::assistant; // 提示词与工具装配
use crate::agent::{agents_repo, chat_prefs, mcp, models, permission}; // 子智能体/偏好/MCP/模型/权限
use crate::config; // 项目路径与配置
use crate::state::AppState; // 全局应用状态
use crate::utils::logger; // 日志输出
pub type EventSender = UnboundedSender<AgentEvent>; // SSE 事件发送器别名
pub const MAX_TITLE_CHARS: usize = 80; // 会话标题截断长度
pub const ASK_TOOL: &str = "ask_user"; // 提问工具名
#[derive(Clone)] // 请求可克隆
pub struct ChatRequest { // 一次对话请求参数
    pub message: String, // 用户消息正文
    pub session_id: String, // 目标会话 id
    pub model: String, // 指定模型名
    pub agent_id: String, // 自定义子智能体 id
    pub resume: String, // 续聊的会话 id
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)] // 结果可打印比较
pub enum Outcome { // 一轮状态机的最终结局
    Done, // 正常完成
    Paused, // 暂停等待用户确认
    Failed, // 失败退出
}
struct PromptOperation { // 系统提示词注入：每步动态拼装最新摘要、关键事实与用户显式激活的 /skill 指引
    base_system: String, // 静态基础提示词（规则/技能/工具清单）
    session_id: String, // 会话 id（空串时不追加会话摘要）
    skill_dir: Option<std::path::PathBuf>, // 技能目录（用于解析用户消息中的 /skill-name）
}
impl PromptOperation { // 提示词操作构造
    fn new(system: &str) -> Self { // 兼容仅静态提示词构造（子智能体内存态用）
        Self { base_system: system.to_string(), session_id: String::new(), skill_dir: None } // 无会话动态摘要
    }
    fn with_session(base_system: String, session_id: String, skill_dir: std::path::PathBuf) -> Self { // 主会话动态注入构造
        Self { base_system, session_id, skill_dir: Some(skill_dir) } // 保存基础提示词、会话 id 与技能目录
    }
}
fn extract_explicit_skill_names(text: &str, enabled_names: &[String]) -> Vec<String> { // 解析文本中显式 /skill-name（仅匹配空白或分隔符后的启用技能，避免误伤文件路径）
    let mut matched = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    for (idx, ch) in chars.iter().enumerate() {
        if *ch != '/' { continue; }
        if idx > 0 {
            let prev = chars[idx - 1];
            if prev.is_alphanumeric() || matches!(prev, '_' | '-' | '.' | '/' | '\\' | ':') { continue; }
        }
        let token: String = chars[idx + 1..].iter().copied().take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')).collect();
        if token.is_empty() { continue; }
        if enabled_names.iter().any(|name| name == &token) && !matched.contains(&token) { matched.push(token); }
    }
    matched
}
fn explicit_skill_suffix(messages: &[Message], skill_dir: Option<&std::path::Path>) -> String { // 读取最新用户文本中的 /skill-name 并生成无副作用系统提示增补（不改 DB 原文）
    let Some(dir) = skill_dir else { return String::new(); };
    let user_text = messages.iter().rev().filter(|m| m.role == rmcp::model::Role::User).find_map(|m| {
        let txt: String = m.content.iter().filter_map(MessageContent::as_text).collect::<Vec<_>>().join("\n");
        if txt.trim().is_empty() { None } else { Some(txt) }
    });
    let Some(prompt) = user_text else { return String::new(); };
    let enabled: Vec<String> = crate::agent::skills::list_skills(dir, "")
        .into_iter()
        .filter(|item| item.get("enabled").and_then(Value::as_bool).unwrap_or(false))
        .filter_map(|item| item.get("name").and_then(Value::as_str).map(str::to_string))
        .collect();
    let hits = extract_explicit_skill_names(&prompt, &enabled);
    if hits.is_empty() { return String::new(); }
    let list_text = hits.iter().map(|s| format!("/{s}")).collect::<Vec<_>>().join("、");
    let has_office = hits.iter().any(|s| matches!(s.as_str(), "word-control" | "ppt-control" | "excel-control"));
    let mut out = format!("\n\n[显式激活技能指令]\n用户在当前请求中显式指定了技能：{list_text}。\n执行具体操作前，必须先逐个调用 `skill_read(name)` 读取上述技能的完整 SKILL.md 正文并严格遵守其中约束。");
    if has_office {
        out.push_str("\n若任务涉及多套题或大批量 Word/PPT/Excel 处理，可先 `agent_list` 并通过 `agent_call` 合理派发 `worduse-agent`、`pptuse-agent`、`exceluse-agent`（所有 Office 工具只接受本机绝对路径）。");
    }
    out
}
#[async_trait] // 异步操作实现
impl Operation<GooseSession, ChatEffect> for PromptOperation { // 实现提示词操作
    fn name(&self) -> &'static str { // 操作名
        "assistant" // 固定名称
    }
    async fn prompt_parts(&self, _session: &GooseSession, conversation: &Conversation) -> Result<Vec<(String, String)>> { // 返回动态拼接后的系统提示片段
        let extra = if self.session_id.is_empty() { String::new() } else { context::inject_text(&self.session_id) }; // 动态拉取最新摘要与事实
        let skill_extra = explicit_skill_suffix(conversation.messages(), self.skill_dir.as_deref()); // 动态解析最新用户消息中的 /skill-name
        Ok(vec![("system".to_string(), format!("{}{extra}{skill_extra}", self.base_system))]) // 仅注入 system 片段
    }
}
struct SessionCompactingProvider { // Provider 包装器：第1层 Microcompact 清理老工具输出 + 第3层超窗被动兜底压缩重试
    inner: Arc<dyn Provider>, // 底层真实模型 Provider
    state: AppState, // 共享数据库状态
    session_id: String, // 当前会话 id
    model: ModelConfig, // 当前模型配置
    base_system: String, // 基础系统提示词（重算动态摘要用）
    tx: EventSender, // SSE 事件发送端
    context_limit: usize, // 模型上下文 Token 上限（用于 Microcompact 比例阈值）
    skill_dir: std::path::PathBuf, // 技能目录（用于被动压缩重试时保留 /skill 激活提示）
    reactive_used: AtomicBool, // 单轮被动超窗重试只触发一次，防无限螺旋
}
impl SessionCompactingProvider { // 构造与包装方法
    fn new(inner: Arc<dyn Provider>, state: AppState, session_id: String, model: ModelConfig, base_system: String, tx: EventSender, context_limit: usize, skill_dir: std::path::PathBuf) -> Arc<dyn Provider> { // 包装为 dyn Provider
        Arc::new(Self { inner, state, session_id, model, base_system, tx, context_limit, skill_dir, reactive_used: AtomicBool::new(false) }) // 返回线程安全句柄
    }
    fn reload_normalized_messages(&self) -> Vec<Message> { // 被动压缩后重新从库读取可见消息并规范化对齐，再套用微压缩
        let conn = self.state.db.lock(); // 短暂加锁读库
        let visible: Vec<Message> = messages_store::list(&conn, &self.session_id).unwrap_or_default().into_iter().filter(|m| m.is_agent_visible()).collect(); // 仅保留模型可见消息
        drop(conn); // 立即释放锁
        let (fixed, _) = fix_conversation(Conversation::new_unvalidated(visible)); // 修复成对工具与角色交替
        let merged = merge_consecutive_messages_for_request(fixed.messages().clone()); // 合并连续同角色消息
        let (projected, _) = context::microcompact_messages_with_limit(&merged, context::MICROCOMPACT_KEEP_RECENT, self.context_limit); // 保护最近 3 个 Assistant 轮次
        projected // 返回精简后请求队列
    }
}
#[async_trait] // 异步 Provider 实现
impl Provider for SessionCompactingProvider { // 委托非流式能力并在流式入口挂载 Microcompact 与被动超窗重试
    fn get_name(&self) -> &str { self.inner.get_name() } // 透传服务商名称
    async fn refresh_credentials(&self) -> Result<(), ProviderError> { self.inner.refresh_credentials().await } // 透传凭证刷新
    async fn fetch_supported_models(&self) -> Result<Vec<String>, ProviderError> { self.inner.fetch_supported_models().await } // 透传模型列表
    async fn stream(&self, model_config: &GooseModelConfig, system: &str, messages: &[Message], tools: &[Tool]) -> Result<MessageStream, ProviderError> { // 拦截流式推理执行 1/3 层压缩
        let ctx_limit = model_config.context_limit.unwrap_or(self.context_limit);
        let (projected, cleared) = context::microcompact_messages_with_limit(messages, context::MICROCOMPACT_KEEP_RECENT, ctx_limit); // 第 1 层：两阶段微压缩
        if cleared > 0 { logger::info("context", &format!("Microcompact 清理历史工具输出: session={} cleared={cleared}", self.session_id)); } // 写入微压缩日志
        match self.inner.stream(model_config, system, &projected, tools).await { // 发起上游推理流
            Err(ProviderError::ContextLengthExceeded(msg)) if !self.reactive_used.swap(true, Ordering::SeqCst) => { // 第 3 层：命中超窗且本轮未曾兜底则压缩并重试一次
                logger::warn("context", &format!("遇到上下文超长错误 ({msg})，触发一次被动兜底压缩: session={}", self.session_id)); // 写入日志告警
                match context::compact_in_turn(&self.state, &self.session_id, &self.model, Some(&self.tx), true).await { // 强制压缩历史并更新摘要
                    Ok(true) => {
                        let next_msgs = self.reload_normalized_messages(); // 重新装载并微压缩消息
                        let next_sys = format!("{}{}{}", self.base_system, context::inject_text(&self.session_id), explicit_skill_suffix(&next_msgs, Some(self.skill_dir.as_path()))); // 动态刷新含最新摘要与 /skill 的系统提示词
                        self.inner.stream(model_config, &next_sys, &next_msgs, tools).await // 使用精简上下文发起单次重试
                    }
                    Ok(false) => Err(ProviderError::ContextLengthExceeded(msg)), // 无可压消息则透传原错误
                    Err(err) => { logger::warn("context", &format!("被动兜底压缩失败: session={} err={err}", self.session_id)); Err(ProviderError::ContextLengthExceeded(msg)) } // 记录失败并透传原错
                }
            }
            other => other, // 正常流或其他错误直接返回
        }
    }
}
struct ApprovalOperation { // 审批与提问：允许直接放行，拒绝写回错误结果，需批准时追加 ActionRequired 并暂停
    routes: HashMap<String, ToolRoute>, // 工具名到路由的映射
    approval: Arc<dyn ToolApproval>, // 审批器
}
impl ApprovalOperation { // 审批操作构造
    fn new(routes: HashMap<String, ToolRoute>) -> Self { // 由路由表构造审批操作
        Self { routes, approval: goose_approval::build_approval() } // 使用默认权限审批器
    }
}
fn tool_error(text: &str) -> Result<CallToolResult, ErrorData> { // 构造工具错误结果
    Ok(CallToolResult::error(vec![ContentBlock::text(text)])) // 包装为 rmcp 错误结果
}
fn deny_text(reason: &str) -> String { // 生成拒绝说明文案
    if reason.is_empty() { "Tool execution was not approved".to_string() } else { format!("Tool execution was not approved：{reason}") } // 有原因时附带原因
}
#[async_trait] // 异步操作实现
impl Operation<GooseSession, ChatEffect> for ApprovalOperation { // 实现审批操作
    fn name(&self) -> &'static str { // 操作名
        "approval" // 固定名称
    }
    async fn run(&self, _session: &GooseSession, conversation: &Conversation, emit: &Emitter) -> Result<OperationResult<ChatEffect>> { // 逐条处理本轮工具请求与确认
        let turn = messages_since_kickoff(conversation)?; // 取本次 kickoff 以来的消息
        let answered: HashSet<String> = turn.iter().flat_map(|item| item.get_tool_response_ids().into_iter().map(str::to_string)).collect(); // 收集已有工具响应的 id
        let decided = goose_session::responses(conversation); // 读取对话中已记录的审批决定
        let mut responses = Message::user(); // 待写回的工具响应消息
        let mut confirmations = Message::assistant(); // 待追加的确认请求消息
        let mut paused = false; // 是否暂停标记
        let mut changed = false; // 本步是否有变更
        for request in turn.iter().flat_map(|item| item.content.iter()).filter_map(MessageContent::as_tool_request) { // 遍历本轮未处理的工具请求
            if answered.contains(&request.id) || request.was_executed_externally() { continue; } // 已响应或外部执行的请求跳过
            let Ok(call) = &request.tool_call else { continue; }; // 工具调用解析失败则跳过
            if let Some(permission) = decided.get(&request.id) { // 已有审批决定时不再重复询问
                if matches!(permission, Permission::Cancel | Permission::DenyOnce | Permission::AlwaysDeny) { // 取消/拒绝时写回错误结果
                    responses = responses.with_tool_response(request.id.clone(), tool_error("Tool execution was not approved")); // 追加未批准错误响应
                    changed = true; // 标记有变更
                }
                continue; // 已有决定直接进入下一条
            }
            let name = call.name.to_string(); // 工具名
            let arguments = call.arguments.clone().unwrap_or_default(); // 工具参数对象
            if name == ASK_TOOL { // ask_user 走提问暂停分支
                let questions = arguments.get("questions").map(|value| crate::state::json_str(Some(value))).unwrap_or_default(); // 提取问题列表文本
                confirmations = confirmations.with_content(MessageContent::action_required(request.id.clone(), name, arguments, Some(questions))); // 构造 ActionRequired 等待用户提问
                paused = true; // 标记本轮暂停
                changed = true; // 标记有变更
                continue; // 进入下一条
            }
            let outcome = self.approval.decide(&name, self.routes.get(&name), &Value::Object(arguments.clone())); // 调用审批器判定 Allow/Deny/Ask
            match outcome.decision { // 按审批决策分派
                ApprovalDecision::Allow => {} // 允许则直接放行
                ApprovalDecision::Deny => { // 拒绝时写回错误
                    responses = responses.with_tool_response(request.id.clone(), tool_error(&deny_text(&outcome.reason))); // 追加拒绝错误响应
                    changed = true; // 标记有变更
                }
                ApprovalDecision::Ask => { // 需批准时追加确认并暂停
                    confirmations = confirmations.with_content(MessageContent::action_required(request.id.clone(), name, arguments, Some(outcome.reason.clone()))); // 追加确认请求
                    paused = true; // 标记暂停
                    changed = true; // 标记有变更
                }
            }
        }
        if !changed { return not_applicable(); } // 无变更时不产生效果
        let mut effects: Vec<ChatEffect> = Vec::new(); // 效果缓冲
        if !responses.content.is_empty() { effects.push(ChatEffect::from(emit.message(responses))); } // 有工具响应时发出消息效果
        if !confirmations.content.is_empty() { effects.push(ChatEffect::from(emit.message(confirmations))); } // 有确认请求时发出消息效果
        if paused { yielded_with(effects) } else { applied(effects) } // 暂停用 yielded_with，否则 applied
    }
}
struct UnhandledToolCalls { // 未注册工具：状态机不再静默跳过，写回工具错误让模型改道
    known: HashSet<String>, // 已注册工具名集合
}
impl UnhandledToolCalls { // 未注册工具操作构造
    fn new(known: HashSet<String>) -> Self { // 由已知工具集合构造
        Self { known } // 保存已知工具名
    }
}
#[async_trait] // 异步操作实现
impl Operation<GooseSession, ChatEffect> for UnhandledToolCalls { // 实现未注册工具检查
    fn name(&self) -> &'static str { // 操作名
        "unhandled" // 固定名称
    }
    async fn run(&self, _session: &GooseSession, conversation: &Conversation, emit: &Emitter) -> Result<OperationResult<ChatEffect>> { // 为未注册工具写回错误让模型改道
        let turn = messages_since_kickoff(conversation)?; // 取本轮消息
        let answered: HashSet<String> = turn.iter().flat_map(|item| item.get_tool_response_ids().into_iter().map(str::to_string)).collect(); // 已响应 id 集合
        let mut message = Message::user(); // 待写回的响应消息
        for request in turn.iter().flat_map(|item| item.content.iter()).filter_map(MessageContent::as_tool_request) { // 遍历本轮工具请求
            if answered.contains(&request.id) || request.was_executed_externally() { continue; } // 已响应或外部执行的请求跳过
            let Ok(call) = &request.tool_call else { continue; }; // 解析失败跳过
            let name = call.name.to_string(); // 工具名
            if self.known.contains(&name) { continue; } // 已注册工具跳过
            message = message.with_tool_response(request.id.clone(), tool_error(&format!("调用失败：未注册的工具 {name}"))); // 写回未注册工具错误
        }
        if message.content.is_empty() { return not_applicable(); } // 无未注册调用则不生效
        applied([ChatEffect::from(emit.message(message))]) // 发出错误响应效果
    }
}
struct MaxTurns { // 轮次上限：达到 max_turns 后结束本轮，避免无限工具循环
    max: i64, // 最大轮次数
}
impl MaxTurns { // 轮次上限操作构造
    fn new(max: i64) -> Self { // 由配置值构造
        Self { max: max.max(1) } // 轮次上限至少为 1
    }
}
#[async_trait] // 异步操作实现
impl Operation<GooseSession, ChatEffect> for MaxTurns { // 实现轮次上限检查
    fn name(&self) -> &'static str { // 操作名
        "max_turns" // 固定名称
    }
    async fn run(&self, _session: &GooseSession, conversation: &Conversation, emit: &Emitter) -> Result<OperationResult<ChatEffect>> { // 达到上限时结束本轮
        let turn = messages_since_kickoff(conversation)?; // 取本轮消息
        if assistant_turn_count(turn) < self.max as u32 { return not_applicable(); } // 未达上限则跳过
        let message = Message::assistant().with_text(format!("已达到最大轮次上限（{}），本轮结束。", self.max)); // 达上限时发消息并结束
        yielded_with([ChatEffect::from(emit.message(message))]) // 以 yielded 结束本轮
    }
}
struct ExitOnError; // 出错退出：对话尾部是错误消息时停止状态机，把错误交给前端展示
#[async_trait] // 异步操作实现
impl Operation<GooseSession, ChatEffect> for ExitOnError { // 实现出错退出检查
    fn name(&self) -> &'static str { // 操作名
        "exit_on_error" // 固定名称
    }
    async fn run(&self, _session: &GooseSession, conversation: &Conversation, _emit: &Emitter) -> Result<OperationResult<ChatEffect>> { // 尾部错误消息时停止
        if conversation.last().and_then(Message::error_kind).is_some() { return yielded::<ChatEffect>(); } // 命中错误则 yielded
        not_applicable() // 无错误继续执行
    }
}
struct MemoryRuntime { // 子智能体运行态：不落库，效果只改内存对话
    session: Mutex<GooseSession>, // 内存会话快照
}
impl MemoryRuntime { // 内存运行态构造与读取
    fn new(session: GooseSession) -> Self { // 由会话快照构造
        Self { session: Mutex::new(session) } // 用互斥锁包裹快照
    }
    fn text(&self) -> String { // 取内存对话中最后的助手文本
        goose_session::last_assistant_text(&self.session.lock().conversation_clone()) // 加锁读取并取文本
    }
}
#[async_trait] // 异步加载实现
impl SessionLoader<GooseSession> for MemoryRuntime { // 从内存快照载入会话
    async fn load(&self, _session_id: &str) -> Result<GooseSession> { // 返回内存会话副本
        Ok(self.session.lock().clone()) // 加锁克隆快照
    }
}
#[async_trait] // 异步效果处理实现
impl EffectHandler<GooseSession, ChatEffect> for MemoryRuntime { // 效果只改内存对话，不写数据库
    async fn apply_effects(&self, _session: &GooseSession, effects: &mut [ChatEffect], _emit: &Emitter) -> Result<()> { // 把效果应用到内存对话
        let mut session = self.session.lock(); // 加锁会话快照
        let mut conversation = session.conversation_clone(); // 取对话副本
        for effect in effects.iter_mut() { // 遍历效果
            if let ChatEffect::Conversation(item) = effect { // 只处理对话变更
                match item { // 按变更类型分派
                    goose_agent::operation::ConversationEffect::AppendMessage(message) => conversation.push(message.clone()), // 追加消息
                    goose_agent::operation::ConversationEffect::ReplaceConversation(next) => conversation = next.clone(), // 替换对话
                    _ => {} // 其余效果在内存模式下忽略
                }
            }
        }
        session.set_conversation(conversation); // 将修改后的对话写回快照
        Ok(()) // 内存效果应用完成
    }
}
fn pick_model(settings: &AgentSettings, model_name: &str) -> Option<ModelConfig> { // 按名称选择模型，未匹配用当前激活或设置里的兜底配置
    let llm = config::load_agent_config().get("llm").cloned().unwrap_or(Value::Null); // 读取 llm 配置
    for item in models::normalize_llm(&llm) { // 遍历归一化模型条目
        let config = models::parse_model(&item); // 解析单条模型配置
        if config.enabled && !config.model.is_empty() && (model_name.is_empty() || config.model == model_name) { return Some(config); } // 命中启用且匹配的模型
    }
    models::active_model(&llm).or_else(|| if settings.api_key.is_empty() { None } else { Some(ModelConfig { provider: "deepseek".to_string(), base_url: settings.base_url.clone(), api_key: settings.api_key.clone(), model: settings.model.clone(), enabled: true, context: "600k".to_string(), thinking: "medium".to_string(), vision: false }) }) // 配置缺失时用设置里的 API Key 兜底
}
fn thinking_of(model: &ModelConfig) -> ThinkingEffort { // 思考强度映射，复用 model_support 统一规则
    models::thinking_effort_of(&model.thinking) // 支持 off/disabled/low/medium/high/max
}
fn reply_error(state: &AppState, session_id: &str, tx: &EventSender, text: &str) { // 构造错误消息、记录日志、落库并经 SSE 发出
    logger::error("state_machine", &format!("对话异常终止 session={session_id}: {text}")); // 记入 app.log 便于排障
    let message = Message::assistant().with_error(MessageErrorKind::Other, text).with_generated_id_if_missing(); // 构造带错误的助手消息
    if !session_id.is_empty() { // 有会话时写入数据库
        let conn = state.db.lock(); // 加锁读取数据库连接
        if sessions::get_session(&conn, session_id).ok().flatten().is_some() { let _ = goose_session::append(&conn, session_id, &message); } // 仅会话存在时追加错误消息
    }
    let _ = tx.send(AgentEvent::Message(message)); // 通过 SSE 发送错误事件
}
fn set_status(state: &AppState, session_id: &str, status: &str) { // 更新会话状态到数据库
    let conn = state.db.lock(); // 加锁读取数据库连接
    let _ = sessions::update_session(&conn, session_id, None, Some(status)); // 状态写回会话表
}
async fn close_runtime(state: &AppState, key: &str) { // 取回并关闭该会话的 MCP 运行时
    if let Some(runtime) = state.take_mcp_runtime(key) { mcp::close_all(&runtime).await; } // 存在则 await 关闭全部连接
}
fn has_pending(state: &AppState, session_id: &str) -> bool { // 判断会话是否存在待用户确认的请求
    let conn = state.db.lock(); // 加锁读取数据库连接
    goose_session::load_conversation(&conn, session_id).map(|conversation| goose_session::has_pending(&conversation)).unwrap_or(false) // 从数据库载入对话后判断
}
fn ensure_session(state: &AppState, request: &ChatRequest, resume: bool) -> Result<String, String> { // 确定会话 id：复用/续聊/新建
    let mut session_id = request.session_id.trim().to_string(); // 请求里的会话 id
    if session_id.is_empty() && resume { session_id = request.resume.trim().to_string(); } // 续聊时回退到 request.resume
    if session_id.is_empty() { // 仍为空则按首条消息新建会话
        let title = crate::state::truncate_chars(&crate::state::collapse(&request.message), MAX_TITLE_CHARS); // 用消息首段生成标题
        let title = if title.is_empty() { "新会话".to_string() } else { title }; // 空标题给默认值
        let conn = state.db.lock(); // 加锁读取数据库连接
        let item = sessions::create_session(&conn, &title, "").map_err(|err| format!("新建会话失败：{err}"))?; // 新建会话并写库
        return Ok(crate::state::json_str(item.get("id"))); // 返回新会话 id
    }
    let conn = state.db.lock(); // 加锁读取数据库连接
    match sessions::get_session(&conn, &session_id) { // 校验会话是否存在
        Ok(Some(_)) => Ok(session_id), // 存在则复用
        Ok(None) => Err(format!("会话不存在: {session_id}")), // 不存在返回错误
        Err(err) => Err(format!("会话读取失败：{err}")), // 读取出错返回错误
    }
}
pub async fn stream_chat(state: AppState, request: ChatRequest, tx: EventSender) -> Outcome { // 主流程：流式对话状态机（含三层上下文压缩与真实 ID 同步）
    let settings = crate::agent::settings::load(); // 读取智能体设置
    if !settings.enabled { // agent 被配置关闭时直接失败
        reply_error(&state, &request.session_id, &tx, "agent.enabled=false，已在配置中关闭"); // 回报关闭原因
        return Outcome::Failed; // 返回失败
    }
    let resume = !request.resume.trim().is_empty(); // 是否续聊
    let session_id = match ensure_session(&state, &request, resume) { // 确定会话 id，失败则报错
        Ok(item) => item, // 会话 id
        Err(err) => { reply_error(&state, &request.session_id, &tx, &err); return Outcome::Failed; } // 会话确定失败
    };
    let Some(_task_guard) = state.try_acquire_task(&session_id) else {
        reply_error(&state, &session_id, &tx, "当前会话正在执行对话或压缩任务，请稍后再试");
        return Outcome::Failed;
    };
    if resume { // 续聊时校验会话非空
        let empty = { let conn = state.db.lock(); messages_store::count(&conn, &session_id).unwrap_or(0) == 0 }; // 检查续聊会话是否为空
        if empty { reply_error(&state, &session_id, &tx, "审批已失效（超时或后端已重启），请重新发起对话"); return Outcome::Failed; } // 消息为空视为审批已失效
    }
    let Some(model) = pick_model(&settings, &request.model) else { // 选择模型，未配置则失败
        reply_error(&state, &session_id, &tx, "未配置可用的模型，请先在设置中填写 API Key"); // 回报未配置
        return Outcome::Failed; // 返回失败
    };
    if model.api_key.trim().is_empty() { // 缺少 API Key 时拒绝执行
        reply_error(&state, &session_id, &tx, "当前模型缺少 API Key，请在设置中填写"); // 回报缺 Key
        return Outcome::Failed; // 返回失败
    }
    if config::project_path().is_none() && !resume { // 未选项目目录且非续聊时拒绝
        reply_error(&state, &session_id, &tx, "请先在顶栏选择项目文件夹后再开始对话"); // 回报未选目录
        return Outcome::Failed; // 返回失败
    }
    let raw_provider = match models::goose_provider(&model) { // 初始化 goose provider
        Ok(item) => item, // 初始化成功
        Err(err) => { reply_error(&state, &session_id, &tx, &format!("模型初始化失败：{err}")); return Outcome::Failed; } // 初始化失败
    };
    let model_config = models::goose_model_config(&model); // 构造 goose 模型配置
    let ctx_limit = model_config.context_limit.unwrap_or(600_000); // 取模型实际上下文上限
    let agent_def = if request.agent_id.trim().is_empty() { None } else { agents_repo::load_agent(request.agent_id.trim()) }; // 加载自定义子智能体定义
    let runtime_key = format!("session:{session_id}"); // MCP 运行时缓存键
    let runtime = match state.mcp_runtime(&runtime_key) { // 取会话级 MCP 运行时
        Some(item) => item, // 复用已有运行时
        None => { let created = Arc::new(mcp::connect_all().await); state.set_mcp_runtime(&runtime_key, created.clone()); created } // 无则新建并缓存，await 连接全部 MCP
    };
    let mcp_defs = if agent_def.is_some() { Vec::new() } else { mcp::tool_defs(&runtime) }; // 子智能体模式不挂主会话 MCP 工具
    let switches = chat_prefs::flags(); // 读取会话级提问/待办开关
    let parent = assistant::ParentContext { db: state.db.clone(), session_id: session_id.clone(), tx: tx.clone() }; // 主会话桥接上下文：子智能体事件写回本会话
    let tools = assistant::build_tools(&settings, agent_def.as_ref(), &switches, mcp_defs, &parent); // 装配工具集
    let base_system = assistant::build_base_system_prompt(&settings, agent_def.as_ref(), &tools, &runtime.catalog, &model.provider); // 构建基础系统提示词
    let mut kicked_id: Option<String> = None; // 记录本轮起始用户消息 ID，供取消时识别是否已被并发回滚删除
    if !resume { // 非续聊才落库用户消息，并立即回推前端同步真实 msg_* ID
        let message = Message::user().with_text(request.message.clone()).with_generated_id_if_missing(); // 构造带唯一 ID 的用户消息
        kicked_id = message.id.clone(); // 保存本次提问 ID
        let appended = { let conn = state.db.lock(); goose_session::append(&conn, &session_id, &message) }; // 用户消息落库
        if let Err(err) = appended { // 落库失败则中止并清理运行时
            reply_error(&state, &session_id, &tx, &format!("消息落库失败：{err}")); // 回报落库失败
            close_runtime(&state, &runtime_key).await; // await 关闭 MCP 运行时
            return Outcome::Failed; // 返回失败
        }
        let _ = tx.send(AgentEvent::Message(message)); // 回推真实用户消息 ID 给前端，确保当轮撤回/删除可直接命中库记录
    }
    let was_rolled_back = |id: &Option<String>| -> bool { // 检查本轮用户提问是否已在回滚接口中被删
        let Some(target) = id.as_deref() else { return false; }; // 续聊不按新建消息判定
        let conn = state.db.lock(); // 加锁读库
        !messages_store::list(&conn, &session_id).unwrap_or_default().iter().any(|m| m.id.as_deref() == Some(target)) // 不存在即已被撤回
    };
    let provider = SessionCompactingProvider::new(raw_provider, state.clone(), session_id.clone(), model.clone(), base_system.clone(), tx.clone(), ctx_limit, settings.skill_dir.clone()); // 挂载微压缩与被动超窗兜底包装器
    let cancel = state.cancel_token(&session_id); // 获取会话取消令牌
    let emit = Emitter::new(tx.clone(), cancel.clone()); // 构造事件发射器
    let known: HashSet<String> = tools.iter().map(|item| item.name.clone()).collect(); // 已注册工具名集合
    let steps = vec![ // 步骤：审批→动态提示→工具→未注册→轮次→推理(带微压缩)→出错退出
        Step::Operation(Arc::new(ApprovalOperation::new(goose_tools::routes_of(&tools)))), // 审批步骤
        Step::Operation(Arc::new(PromptOperation::with_session(base_system, session_id.clone(), settings.skill_dir.clone()))), // 动态注入最新摘要、关键事实与 /skill 激活提示
        Step::Operation(Arc::new(goose_tools::build_goose_tool_operation::<GooseSession>(tools.clone()))), // 工具执行步骤
        Step::Operation(Arc::new(UnhandledToolCalls::new(known))), // 未注册工具检查步骤
        Step::Operation(Arc::new(MaxTurns::new(settings.max_turns))), // 轮次上限步骤
        Step::Inference(Arc::new(InferenceRunner::<GooseSession, ChatEffect>::new(provider, model_config))), // 推理步骤：调用模型
        Step::Operation(Arc::new(ExitOnError)), // 出错退出步骤
    ];
    let machine = StateMachine::new(steps, cancel.clone()); // 创建状态机
    let store = SessionStore::new(state.db.clone(), &session_id, Some(thinking_of(&model))); // 会话存储：落 SQLite + 思考强度
    let outcome = loop { // 逐步推进状态机
        if cancel.is_cancelled() { // 取消令牌触发时仅未回滚才收尾写入
            if !was_rolled_back(&kicked_id) { let _ = machine.finalize(&store, &session_id, &emit).await; } // 已撤回时不回写残余数据
            break Outcome::Failed; // 取消退出
        }
        context::maybe_auto_compact(&state, &session_id, &model, ctx_limit, &tx).await; // 第 2 层：每步载入前检查 80% 累计增量触发自动压缩
        let session = match store.load(&session_id).await { // await 从 SQLite 载入最新会话（含刚隐藏后的可见消息）
            Ok(item) => item, // 载入成功
            Err(err) => { reply_error(&state, &session_id, &tx, &format!("会话读取失败：{err}")); break Outcome::Failed; } // 载入失败
        };
        let mut result = match machine.step(&session, &emit).await { // await 推进一步
            Ok(item) => item, // 推进成功
            Err(err) => {
                if !was_rolled_back(&kicked_id) {
                    let _ = machine.finalize(&store, &session_id, &emit).await; // 收尾
                    reply_error(&state, &session_id, &tx, &format!("本轮执行失败：{err}")); // 上报错误
                }
                break Outcome::Failed; // 失败退出
            }
        };
        for effect in &result.effects { // 记录推理或工具产出的错误内容到 app.log
            if let ChatEffect::Conversation(ConversationEffect::AppendMessage(msg)) = effect { // 检索追加消息效果
                if msg.error_kind().is_some() { logger::error("state_machine", &format!("模型推理报错 session={session_id} model={}: {}", model.model, serde_json::to_string(&msg.content).unwrap_or_default())); } // 写入详细错误文本
            }
        }
        let status = result.status; // 取本步运行状态
        if cancel.is_cancelled() && was_rolled_back(&kicked_id) { break Outcome::Failed; } // 步内并发触发回滚时不落库多余效果
        if let Err(err) = machine.apply(&store, &session, &mut result.effects, &emit).await { // await 将本步效果落库
            reply_error(&state, &session_id, &tx, &format!("消息落库失败：{err}")); // 回报落库失败
            break Outcome::Failed; // 返回失败
        }
        let paused = has_pending(&state, &session_id); // 检查是否出现待确认请求
        match status { // 按运行状态决定继续/暂停/完成
            RunStatus::Continuing => { if paused { break Outcome::Paused; } } // 继续执行但有待确认则暂停
            RunStatus::Yielded => { break if paused { Outcome::Paused } else { Outcome::Done }; } // Yielded 时按是否有待确认区分暂停/完成
            RunStatus::Cancelled => {
                if !was_rolled_back(&kicked_id) { let _ = machine.finalize(&store, &session_id, &emit).await; } // 未回滚才收尾
                break Outcome::Failed; // 取消退出
            }
        }
    };
    match outcome { // 按结局做收尾处理
        Outcome::Paused => { state.clear_cancel(&session_id); logger::info("state_machine", &format!("会话暂停等待用户确认: {session_id}")); } // 暂停等待用户确认时释放取消令牌，避免阻塞清库预览
        Outcome::Done => { // 完成时更新会话状态
            set_status(&state, &session_id, "completed"); // 标记完成
            close_runtime(&state, &runtime_key).await; // await 关闭 MCP 运行时
            state.clear_cancel(&session_id); // 清理取消令牌
        }
        Outcome::Failed => { // 失败或取消时保护并发回滚后的准确状态
            if was_rolled_back(&kicked_id) { let conn = state.db.lock(); let _ = messages_store::recompute_session_status(&conn, &session_id); } // 已撤回则按剩余消息重算状态
            else if cancel.is_cancelled() { set_status(&state, &session_id, "interrupted"); } // 用户主动停止记为中断
            else { set_status(&state, &session_id, "failed"); } // 真实失败记为异常
            close_runtime(&state, &runtime_key).await; // await 关闭 MCP 运行时
            state.clear_cancel(&session_id); // 清理取消令牌
        }
    }
    outcome // 返回最终结局
}
fn spawn_forward(rx: UnboundedReceiver<AgentEvent>, run: assistant::SubagentRun) -> tokio::task::JoinHandle<()> { // 子智能体事件转发：补元数据→落主会话→实时推送
    tokio::spawn(async move { // 后台消费子智能体事件
        let mut rx = rx; // 事件接收端
        let mut seq: i64 = 0; // 子消息序号
        while let Some(event) = rx.recv().await { // 逐事件消费
            let AgentEvent::Message(message) = event else { continue; }; // 只转发消息事件，用量等事件不落主会话
            seq += 1; // 自增序号
            let mut message = message.with_generated_id_if_missing(); // 补齐消息 id
            message.metadata.user_visible = true; // 前端可见
            message.metadata.agent_visible = false; // 不进入主智能体上下文
            message.metadata.set_operation_note("subagent", "callId", Value::String(run.call_id.clone())); // 记录派发 id
            message.metadata.set_operation_note("subagent", "agentId", Value::String(run.agent_id.clone())); // 记录子智能体 id
            message.metadata.set_operation_note("subagent", "agentName", Value::String(run.agent_name.clone())); // 记录子智能体名称
            message.metadata.set_operation_note("subagent", "seq", Value::from(seq)); // 记录子消息序号
            { let conn = run.parent.db.lock(); let _ = goose_session::append_merged(&conn, &run.parent.session_id, &message); } // 落主会话（文本增量按同 id 合并）
            let _ = run.parent.tx.send(AgentEvent::Message(message)); // 经主会话 SSE 通道实时推送
        }
    })
}
pub async fn run_task(settings: &AgentSettings, provider: &str, instructions: &str, tools: Vec<ToolDef>, task: &str, max_turns: i64, bridge: Option<assistant::SubagentRun>) -> Result<String, String> { // 子智能体任务：内存状态机执行，事件按需转发主会话
    let _ = provider; // provider 参数保留兼容，当前按配置解析
    let llm = config::load_agent_config().get("llm").cloned().unwrap_or(Value::Null); // 读取 llm 配置
    let Some(model) = models::active_model(&llm) else { return Err("未配置可用模型".to_string()); }; // 未配置可用模型直接失败
    let provider_impl = models::goose_provider(&model)?; // 构造 goose provider
    let model_config = models::goose_model_config(&model); // 构造模型配置
    let conversation = Conversation::new_unvalidated([Message::user().with_text(task)]); // 以任务描述构造初始对话
    let runtime = MemoryRuntime::new(GooseSession::new("subagent", conversation, Some(thinking_of(&model)))); // 使用内存运行态，不落库
    let cancel = CancellationToken::new(); // 子智能体独立取消令牌
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<AgentEvent>(); // 创建事件通道
    let emit = Emitter::new(tx, cancel.clone()); // 构造事件发射器
    let forwarder = bridge.map(|run| spawn_forward(rx, run)); // 有桥接上下文时启动事件转发任务
    let known: HashSet<String> = tools.iter().map(|item| item.name.clone()).collect(); // 已注册工具名集合
    let steps = vec![ // 步骤：提示→工具(带审批)→未注册→轮次→推理→出错退出
        Step::Operation(Arc::new(PromptOperation::new(instructions))), // 提示词注入步骤
        Step::Operation(Arc::new(goose_tools::build_goose_tool_operation_with_approval::<GooseSession>(tools.clone(), goose_approval::build_approval()))), // 工具步骤同样经过审批门
        Step::Operation(Arc::new(UnhandledToolCalls::new(known))), // 未注册工具检查
        Step::Operation(Arc::new(MaxTurns::new(max_turns))), // 轮次上限
        Step::Inference(Arc::new(InferenceRunner::<GooseSession, ChatEffect>::new(provider_impl, model_config))), // 推理步骤：调用模型
        Step::Operation(Arc::new(ExitOnError)), // 出错退出
    ];
    let machine = StateMachine::new(steps, cancel.clone()); // 创建状态机
    let job = machine.run(&runtime, "subagent", &emit); // 启动整段状态机，交给超时控制
    let outcome = tokio::time::timeout(Duration::from_secs(settings.timeout_seconds.max(10) as u64), job).await; // await 并对整体套超时
    drop(emit); // 释放事件发送端，让转发任务消费完剩余事件后退出
    if let Some(handle) = forwarder { let _ = handle.await; } // 等待转发任务收尾
    match outcome { // 按执行结果返回
        Ok(Ok(_)) => Ok(runtime.text()), // 成功返回内存对话中的助手文本
        Ok(Err(err)) => Err(err.to_string()), // 执行失败上抛错误
        Err(_) => Err("子智能体执行超时".to_string()), // 整体超时转中文错误
    }
}
pub fn approve_entry(tool: &str, arguments: &Value) -> String { // 从参数中提取可白名单的命令条目
    for key in ["command", "cmd", "code", "script"] { // 依次尝试常见命令字段
        if let Some(text) = arguments.get(key).and_then(Value::as_str) { if !text.trim().is_empty() { return text.trim().to_string(); } } // 取首个非空命令文本
    }
    tool.to_string() // 无命令时以工具名作为条目
}
pub fn pending_confirmations(state: &AppState, session_id: &str) -> Vec<Value> { // 读取待用户确认列表
    let conn = state.db.lock(); // 加锁读取数据库连接
    goose_session::load_conversation(&conn, session_id).map(|conversation| goose_session::pending_actions(&conversation)).unwrap_or_default() // 从对话中提取待确认动作
}
pub fn apply_answers(arguments: &JsonObject, answers: &Value) { // 把用户回答写回待提问状态
    let questions = arguments.get("questions").map(|value| crate::state::json_str(Some(value))).unwrap_or_default(); // 取问题文本
    if !questions.is_empty() { set_pending_answers(&questions, answers); } // 有问题时才设置回答
}
pub fn remember_command(tool: &str, arguments: &Value) { // 记住已批准的命令条目
    let entry = approve_entry(tool, arguments); // 提取白名单条目
    if !entry.is_empty() { permission::append_custom(&entry); } // 写回自定义白名单
}
#[cfg(test)]
mod tests {
    use super::extract_explicit_skill_names;
    #[test]
    fn extracts_only_boundary_matched_enabled_skills() {
        let enabled = vec!["excel-control".into(), "ppt-control".into(), "word-control".into()];
        assert_eq!(
            extract_explicit_skill_names("分析后调用 /excel-control /ppt-control，/word-control，忽略 office_titles/word-control/a.md 与 disabled /unknown-skill", &enabled),
            vec!["excel-control", "ppt-control", "word-control"]
        );
        assert!(extract_explicit_skill_names("office_titles/word-control/readme.md", &enabled).is_empty());
    }
}
