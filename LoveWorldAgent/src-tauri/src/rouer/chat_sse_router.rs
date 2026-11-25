use std::collections::HashMap; // 对话路由：/api/chat/sse 设置与提示音、/stream SSE 直出 goose AgentEvent、/stop 真实中断、/compress 上下文压缩、/approve 审批与作答
use axum::body::{Body, Bytes}; // 响应体与请求体
use axum::extract::{Path, State}; // 路径参数与状态提取
use axum::http::{header, StatusCode}; // 响应头与状态码
use axum::response::{IntoResponse, Response}; // 响应 trait
use axum::routing::{get, post}; // GET/POST 方法
use axum::Router; // 路由器类型
use goose_agent::events::AgentEvent; // goose 事件类型
use goose_provider_types::conversation::message::{Message, MessageContent}; // 消息与内容块
use goose_provider_types::permission::Permission; // 审批权限值
use serde_json::{json, Value}; // JSON 构造
use crate::agent::agent_core::goose_session; // 会话消息落库
use crate::agent::agent_core::goose_approval; // 子智能体审批桥唤醒
use crate::agent::agent_core::state_machine::{self, ASK_TOOL, ChatRequest}; // 状态机主流程
use crate::agent::store_sqls::{messages as messages_store, sessions}; // 会话与消息数据库
use crate::agent::agent_core::context; // 上下文压缩
use crate::agent::chat_prefs; // 对话设置
use crate::config; // 配置路径与读写
use crate::state::{body_object, fail, json_str, ok, ApiResult, AppState}; // 请求解析与响应工具
use crate::utils::logger; // 日志输出
pub const SOUND_FILES: [(&str, &str); 3] = [("win", "tishi_vedio_win.mp3"), ("wait", "tishi_vedio_wait.mp3"), ("error", "tishi_vedio_error.mp3")]; // 提示音文件映射
pub const DECISIONS: [&str; 4] = ["once", "always", "reject", "answer"]; // 合法审批决策
pub fn router() -> Router<AppState> { // 注册对话路由
    Router::new() // 创建路由器
        .route("/api/chat/sse", get(get_chat).put(save_chat)) // 对话设置读写
        .route("/api/chat/sse/sound/{name}", get(sound)) // 提示音文件
        .route("/api/chat/stream", post(stream)) // SSE 对话流
        .route("/api/chat/stop", post(stop)) // 真实中断本轮
        .route("/api/chat/compress", post(compress)) // 手动上下文压缩
        .route("/api/chat/approve", post(approve)) // 审批与作答
}
pub fn event_json(event: &AgentEvent) -> Value { // 把 goose 事件转为前端 JSON
    match event { // 按事件类型分派
        AgentEvent::Message(message) => json!({"type": "message", "message": message}), // 消息事件
        AgentEvent::Usage(usage) => json!({"type": "usage", "usage": usage}), // 用量事件
        AgentEvent::MessageUsage { message_id, usage } => json!({"type": "messageUsage", "messageId": message_id, "usage": usage}), // 消息级用量
        AgentEvent::McpNotification((server, notification)) => json!({"type": "mcpNotification", "server": server, "notification": notification}), // MCP 通知
        AgentEvent::HistoryReplaced(conversation) => json!({"type": "historyReplaced", "messages": conversation.messages()}), // 历史替换
    }
}
fn sse_frame(event: &AgentEvent) -> Vec<u8> { // 序列化为 SSE data 帧
    format!("data: {}\n\n", serde_json::to_string(&event_json(event)).unwrap_or_else(|_| "{}".to_string())).into_bytes() // data 行 + 空行
}
fn sse_response(receiver: tokio::sync::mpsc::UnboundedReceiver<AgentEvent>) -> Response { // 将事件通道封装为标准 SSE HTTP 响应
    let body_stream = futures_util::stream::unfold(receiver, |mut rx| async move {
        rx.recv().await.map(|event| (Ok::<Vec<u8>, std::convert::Infallible>(sse_frame(&event)), rx))
    });
    Response::builder().status(StatusCode::OK).header(header::CONTENT_TYPE, "text/event-stream").header(header::CACHE_CONTROL, "no-cache").header(header::CONNECTION, "keep-alive").header("X-Accel-Buffering", "no").body(Body::from_stream(body_stream)).unwrap_or_else(|_| fail(500, "SSE 响应初始化失败").into_response())
}
async fn get_chat() -> ApiResult { // 读取对话设置
    ok(chat_prefs::read_chat()) // 返回设置
}
fn write_chat(chat: &Value) -> anyhow::Result<()> { // 写回对话设置
    let source = config::resolve_config_file().unwrap_or_else(|| config::WRITABLE_CONFIG_FILE.clone()); // 选取配置来源
    let mut data = config::read_json(Some(source.as_path())); // 读取配置
    if !data.is_object() { data = json!({}); } // 非对象重置
    data["chat"] = chat.clone(); // 写入 chat 段
    config::atomic_write_json(&config::WRITABLE_CONFIG_FILE, &data) // 原子写入
}
async fn save_chat(body: Bytes) -> ApiResult { // 保存对话设置
    let payload = body_object(&body); // 解析请求体
    let chat = chat_prefs::normalize(&payload); // 归一化设置
    if let Err(err) = write_chat(&chat) { // 写入配置
        logger::error("chat_sse_router", &format!("保存对话设置失败: {err}")); // 记录错误
        return fail(500, "写入配置失败"); // 返回失败
    }
    logger::info("chat_sse_router", &format!("对话设置已保存: ask={} todo={}", chat.get("ask").and_then(Value::as_bool).unwrap_or(true), chat.get("todo").and_then(Value::as_bool).unwrap_or(true))); // 记录成功
    ok(chat) // 返回设置
}
async fn sound(Path(name): Path<String>) -> Response { // 返回提示音文件
    let key = name.trim().to_lowercase(); // 归一化名称
    let Some((_, file)) = SOUND_FILES.iter().find(|(item, _)| *item == key) else { return fail(404, "提示音不存在").into_response(); }; // 未登记返回 404
    let path = config::LOVEWORD_DIR.join("vedios").join(file); // 文件路径
    if !path.exists() { return fail(404, &format!("提示音文件缺失: {file}")).into_response(); } // 文件缺失
    match tokio::fs::read(&path).await { // 异步读取
        Ok(bytes) => Response::builder().status(StatusCode::OK).header(header::CONTENT_TYPE, "audio/mpeg").header(header::CACHE_CONTROL, "no-store").body(Body::from(bytes)).unwrap_or_else(|_| fail(500, "读取提示音失败").into_response()), // 返回音频
        Err(_) => fail(500, "读取提示音失败").into_response(), // 读取失败
    }
}
fn history_of(payload: &Value) -> Vec<Value> { // 提取历史消息，只保留 user/assistant
    payload.get("history").and_then(Value::as_array).map(|items| items.iter().filter(|item| matches!(item.get("role").and_then(Value::as_str), Some("user") | Some("assistant"))).map(|item| json!({"role": item.get("role").cloned().unwrap_or(Value::Null), "content": item.get("content").cloned().unwrap_or(Value::Null)})).collect()).unwrap_or_default() // 投影 role 与 content
}
fn int_of(payload: &Value, key: &str) -> i64 { // 读取整数字段，兼容字符串
    payload.get(key).and_then(|value| match value { Value::Number(number) => number.as_i64(), Value::String(text) => text.trim().parse::<i64>().ok(), _ => None }).unwrap_or(0) // 缺省 0
}
async fn stream(State(state): State<AppState>, body: Bytes) -> Response { // SSE 对话流主入口（自动压缩已下沉至状态机每步推理前执行，支持反复触发）
    let payload = body_object(&body); // 解析请求体
    let message = json_str(payload.get("message")).trim().to_string(); // 用户消息
    let resume = json_str(payload.get("resume")).trim().to_string(); // 续聊 id
    if message.is_empty() && resume.is_empty() { return fail(400, "message 不能为空").into_response(); } // 两者皆空
    let session_id = json_str(payload.get("session_id")).trim().to_string(); // 会话 id
    let check_sid = if !session_id.is_empty() { session_id.as_str() } else { resume.as_str() };
    if !check_sid.is_empty() && state.has_active_task(check_sid) { return fail(409, "当前会话正在执行对话或压缩任务，请稍后再试").into_response(); } // 并发守卫
    let request = ChatRequest { // 构造聊天请求
        message, // 用户消息
        session_id, // 会话 id
        model: json_str(payload.get("model")).trim().to_string(), // 模型
        agent_id: json_str(payload.get("agent")).trim().to_string(), // 子智能体 id
        resume, // 续聊 id
    };
    let (tx, receiver) = tokio::sync::mpsc::unbounded_channel::<AgentEvent>(); // 事件通道
    let task_state = state.clone(); // 克隆状态供后台任务
    tokio::spawn(async move { // 后台运行状态机
        state_machine::stream_chat(task_state, request, tx).await; // await 主流程（含轮内三层上下文压缩）
    });
    sse_response(receiver) // 返回 SSE 响应
}
async fn compress(State(state): State<AppState>, body: Bytes) -> Response { // 手动压缩上下文：SSE 渐进推送 + 并发 409 防重锁 + 落库助手消息
    let payload = body_object(&body); // 解析请求体
    let session_id = json_str(payload.get("session_id")).trim().to_string(); // 会话 id
    if session_id.is_empty() { return fail(400, "session_id 不能为空").into_response(); } // 校验
    let has_session = {
        let conn = state.db.lock();
        match sessions::get_session(&conn, &session_id) {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(err) => return fail(500, &format!("读取会话失败: {err}")).into_response(),
        }
    };
    if !has_session { return fail(404, &format!("会话不存在: {session_id}")).into_response(); }
    let Some(guard) = state.try_acquire_task(&session_id) else {
        return fail(409, "当前会话正在执行对话或上下文压缩，请稍后再试").into_response();
    };
    let history = history_of(&payload);
    let has_db = { let conn = state.db.lock(); messages_store::count(&conn, &session_id).unwrap_or(0) > 1 };
    if !has_db && history.is_empty() {
        drop(guard);
        return fail(400, "当前会话暂无可压缩历史，请先发送至少一轮对话").into_response();
    }
    let tokens = int_of(&payload, "context_tokens"); // 当前 token
    let ctx_limit = int_of(&payload, "context_limit").max(0) as usize; // 当前模型上下文上限
    let model = json_str(payload.get("model")).trim().to_string(); // 模型
    let agent = json_str(payload.get("agent")).trim().to_string(); // 子智能体
    let (tx, receiver) = tokio::sync::mpsc::unbounded_channel::<AgentEvent>();
    let task_state = state.clone();
    tokio::spawn(async move {
        let _task_guard = guard;
        context::compress_stream(task_state, session_id, history, model, agent, tokens, ctx_limit, tx).await;
    });
    sse_response(receiver)
}
fn session_of(state: &AppState, run_id: &str) -> Option<String> { // 由 run_id 查会话 id
    let conn = state.db.lock(); // 加锁读取数据库
    sessions::get_session(&conn, run_id).ok().flatten().map(|item| json_str(item.get("id"))).filter(|text| !text.is_empty()) // 取非空 id
}
async fn approve(State(state): State<AppState>, body: Bytes) -> ApiResult { // 写入审批决策恢复对话
    let payload = body_object(&body); // 解析请求体
    let run_id = json_str(payload.get("run_id")).trim().to_string(); // 运行 id
    let decision = json_str(payload.get("decision")).trim().to_lowercase(); // 决策类型
    let answers = payload.get("answers").filter(|value| value.is_array()).cloned().unwrap_or_else(|| json!([])); // 用户答案
    if run_id.is_empty() { return fail(400, "run_id 不能为空"); } // 校验
    if !DECISIONS.contains(&decision.as_str()) { return fail(400, "decision 必须是 once/always/reject/answer"); } // 决策校验
    let Some(session_id) = session_of(&state, &run_id) else { return fail(404, &format!("审批已失效或不存在: {run_id}")); }; // 会话不存在
    let pending = state_machine::pending_confirmations(&state, &session_id); // 取待确认项
    if pending.is_empty() { return fail(409, "该审批已无待决项，请重新发起对话"); } // 无待决项
    let mut message = Message::user().with_visibility(false, false); // 构造不可见确认消息
    let mut matched = false; // 是否命中
    let mut resolved: Vec<String> = Vec::new(); // 已唤醒的子智能体审批 id
    for item in &pending { // 逐项处理
        let id = json_str(item.get("id")); // 确认 id
        let tool = json_str(item.get("toolName")); // 工具名
        let arguments = item.get("arguments").cloned().unwrap_or_else(|| json!({})); // 工具参数
        if decision == "answer" { // 回答问题分支
            if tool != ASK_TOOL { continue; } // 只处理提问工具
            let fields = arguments.as_object().cloned().unwrap_or_default(); // 问题参数
            state_machine::apply_answers(&fields, &answers); // 记住答案
            message = message.with_content(MessageContent::action_required_tool_confirmation_response(id, Permission::AllowOnce)); // 写入允许响应
            matched = true; // 标记命中
            continue; // 下一项
        }
        if decision == "always" { state_machine::remember_command(&tool, &arguments); } // 总是允许则记住命令
        let permission = if decision == "once" || decision == "always" { Permission::AllowOnce } else { Permission::Cancel }; // 映射权限值
        if goose_approval::resolve_subagent(&id, permission.clone()) { resolved.push(id.clone()); } // 命中子智能体审批则唤醒其工具调用
        message = message.with_content(MessageContent::action_required_tool_confirmation_response(id, permission)); // 写入响应
        matched = true; // 标记命中
    }
    if !matched { return fail(409, "该审批与决策类型不匹配，请重新发起对话"); } // 无匹配项
    let message = message.with_generated_id_if_missing(); // 补消息 id
    let appended = { let conn = state.db.lock(); goose_session::append(&conn, &session_id, &message) }; // 落库
    if let Err(err) = appended { // 落库失败
        logger::error("chat_sse_router", &format!("审批落库失败: {session_id} {err}")); // 记录错误
        return fail(500, "审批落库失败"); // 返回失败
    }
    if !resolved.is_empty() { // 命中子智能体审批时直接返回，无需恢复主会话
        logger::info("chat_sse_router", &format!("子智能体审批决策: session={session_id} decision={decision} ids={resolved:?}")); // 记录成功
        return ok(json!({"run_id": session_id, "decision": decision, "subagent": true, "resolved": resolved})); // 返回子智能体审批结果
    }
    logger::info("chat_sse_router", &format!("审批决策: session={session_id} decision={decision}")); // 记录成功
    ok(json!({"run_id": session_id, "decision": decision, "resumed": true})) // 返回结果
}
async fn stop(State(state): State<AppState>, body: Bytes) -> ApiResult { // 真实中断本轮
    let payload = body_object(&body); // 解析请求体
    let run_id = json_str(payload.get("run_id")).trim().to_string(); // 运行 id（前端传会话 id）
    if run_id.is_empty() { return fail(400, "run_id 不能为空"); } // 校验
    let Some(session_id) = session_of(&state, &run_id) else { return fail(404, &format!("会话不存在: {run_id}")); }; // 会话必须存在
    state.stop(&session_id); // 触发取消令牌
    logger::info("chat_sse_router", &format!("已请求停止本轮: session={session_id}")); // 记录
    ok(json!({"run_id": run_id, "stopped": true})) // 返回结果
}
pub fn sound_files() -> HashMap<String, String> { // 对外暴露提示音映射
    SOUND_FILES.iter().map(|(key, file)| (key.to_string(), file.to_string())).collect() // 转为 HashMap
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn compress_guards_missing_session_empty_history_and_concurrent_run_with_409() {
        let app = AppState::memory();
        let sid = {
            let conn = app.db.lock();
            sessions::create_session(&conn, "test-compress", "").unwrap()["id"].as_str().unwrap().to_string()
        };
        let r_missing = compress(State(app.clone()), Bytes::from_static(br#"{"session_id":"no-such"}"#)).await;
        assert_eq!(r_missing.status(), StatusCode::NOT_FOUND);
        let r_empty = compress(State(app.clone()), Bytes::from(json!({"session_id": sid}).to_string())).await;
        assert_eq!(r_empty.status(), StatusCode::BAD_REQUEST);
        let guard = app.try_acquire_task(&sid).expect("first task acquire succeeds");
        assert!(app.has_active_task(&sid));
        let r_conflict = compress(State(app.clone()), Bytes::from(json!({"session_id": sid, "history": [{"role": "user", "content": "hi"}]}).to_string())).await;
        assert_eq!(r_conflict.status(), StatusCode::CONFLICT);
        drop(guard);
        assert!(!app.has_active_task(&sid));
        assert!(app.try_acquire_task(&sid).is_some());
    }
}
