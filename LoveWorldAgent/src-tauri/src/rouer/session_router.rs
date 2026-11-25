use std::collections::HashMap; // 会话与消息接口：goose 消息读写，支持列表、新建、重命名/改状态、删除与消息/轮次/用量读写
use axum::body::Bytes; // 请求体类型
use axum::extract::{Path, Query, State}; // 路径/查询参数与状态提取
use axum::routing::{delete, get, patch}; // GET/PATCH/DELETE 方法
use axum::Router; // 路由器类型
use goose_provider_types::conversation::message::Message; // 消息类型
use serde_json::{json, Value}; // JSON 构造
use crate::agent::store_sqls::{context, messages, sessions, turns, usage}; // 数据库读写模块
use crate::state::{body_object, collapse, fail, json_str, ok, query_int, truncate_chars, ApiResult, AppState}; // 请求解析与响应工具
pub const STATUSES: [&str; 4] = ["running", "completed", "interrupted", "failed"]; // 合法会话状态
pub const ROLES: [&str; 2] = ["user", "assistant"]; // 合法消息角色
pub fn router() -> Router<AppState> { // 注册会话路由
    Router::new() // 创建路由器
        .route("/api/sessions", get(list_sessions).post(create_session)) // 列举与新建
        .route("/api/sessions/{session_id}", patch(update_session).delete(delete_session)) // 更新/删除
        .route("/api/sessions/{session_id}/messages", get(list_messages).post(append_message)) // 消息列举与追加
        .route("/api/sessions/{session_id}/messages/{turn_key}", delete(delete_message_turn)) // 按轮次删消息（支持 ?after=1 回滚）
        .route("/api/sessions/{session_id}/usage", get(session_usage)) // 用量查询
        .route("/api/sessions/{session_id}/context", get(session_context)) // 压缩摘要与关键事实查询
        .route("/api/sessions/{session_id}/turns", get(list_turns).post(append_turn)) // 轮次列举与追加
        .route("/api/sessions/{session_id}/turns/{turn_key}", patch(update_turn).delete(delete_turn)) // 轮次更新/删除
}
fn db_error(err: impl std::fmt::Display) -> ApiResult { // 统一数据库错误处理
    crate::utils::logger::error("session_router", &format!("数据库操作失败: {err}")); // 记录错误
    fail(500, "数据库操作失败") // 返回 500
}
fn text_of(payload: &Value) -> String { // 兼容 content 与 parts 两种消息文本格式
    if let Some(text) = payload.get("content").and_then(Value::as_str) { return text.to_string(); } // content 字段
    match payload.get("parts") { // parts 字段
        Some(Value::String(text)) => text.clone(), // 字符串形式
        Some(Value::Array(items)) => items.iter().filter_map(|item| item.get("text").and_then(Value::as_str)).collect::<Vec<&str>>().join("\n"), // 数组取 text 拼接
        _ => String::new(), // 其他为空
    }
}
async fn list_sessions(State(state): State<AppState>, Query(query): Query<HashMap<String, String>>) -> ApiResult { // 列出会话，支持关键字
    let keyword = query.get("q").cloned().unwrap_or_default(); // 查询关键字
    match sessions::list_sessions(&state.db.lock(), &keyword) { // 查询数据库
        Ok(items) => ok(json!({"total": items.len(), "list": items})), // 成功返回
        Err(err) => db_error(err), // 失败返回错误
    }
}
async fn create_session(State(state): State<AppState>, body: Bytes) -> ApiResult { // 新建会话
    let payload = body_object(&body); // 解析请求体
    let raw = payload.get("title").map(|value| json_str(Some(value))).unwrap_or_default(); // 标题
    let title = truncate_chars(&collapse(if raw.is_empty() { "新会话" } else { &raw }), 80); // 压缩空白并截断到 80
    match sessions::create_session(&state.db.lock(), &title, "") { // 插入会话
        Ok(item) => ok(item), // 成功返回
        Err(err) => db_error(err), // 失败返回错误
    }
}
async fn update_session(State(state): State<AppState>, Path(session_id): Path<String>, body: Bytes) -> ApiResult { // 更新会话标题或状态
    let payload = body_object(&body); // 解析请求体
    let mut title: Option<String> = None; // 新标题
    let mut status: Option<String> = None; // 新状态
    if payload.get("title").is_some() { // 提交标题
        let text = truncate_chars(&collapse(&json_str(payload.get("title"))), 80); // 归一化
        if text.is_empty() { return fail(400, "会话标题不能为空"); } // 空标题
        title = Some(text); // 写入标题
    }
    if payload.get("status").is_some() { // 提交状态
        let text = payload.get("status").and_then(Value::as_str).unwrap_or_default().trim().to_lowercase(); // 状态值
        if !STATUSES.contains(&text.as_str()) { return fail(400, "status 必须是 running/completed/interrupted/failed"); } // 状态校验
        status = Some(text); // 写入状态
    }
    match sessions::update_session(&state.db.lock(), &session_id, title.as_deref(), status.as_deref()) { // 更新数据库
        Ok(Some(item)) => ok(item), // 成功返回
        Ok(None) => fail(404, &format!("会话不存在: {session_id}")), // 不存在
        Err(err) => db_error(err), // 失败
    }
}
async fn delete_session(State(state): State<AppState>, Path(session_id): Path<String>) -> ApiResult { // 删除会话
    state.stop(&session_id); // 停止关联会话流
    state.clear_cancel(&session_id); // 释放取消令牌，避免遗留阻塞清库
    if let Some(runtime) = state.take_mcp_runtime(&format!("session:{session_id}")) { crate::agent::mcp::close_all(&runtime).await; } // 清理会话缓存的 MCP 连接
    match sessions::delete_session(&state.db.lock(), &session_id) { // 级联删除
        Ok(true) => ok(json!({"id": session_id})), // 成功返回
        Ok(false) => fail(404, &format!("会话不存在: {session_id}")), // 不存在
        Err(err) => db_error(err), // 失败
    }
}
async fn list_messages(State(state): State<AppState>, Path(session_id): Path<String>, Query(query): Query<HashMap<String, String>>) -> ApiResult { // 分页列出消息
    let conn = state.db.lock(); // 加锁读取数据库
    match sessions::get_session(&conn, &session_id) { // 校验会话存在
        Ok(None) => return fail(404, &format!("会话不存在: {session_id}")), // 不存在
        Err(err) => return db_error(err), // 失败
        _ => {} // 存在
    }
    let after_seq = query_int(&query, "after_seq", 0).max(0); // 起始序号
    let limit = query_int(&query, "limit", 50); // 分页大小
    let limit = if limit > 0 { limit } else { 50 }; // 缺省 50
    match (messages::list_page(&conn, &session_id, after_seq, limit), messages::count(&conn, &session_id)) { // 查询分页与总数
        (Ok(items), Ok(total)) => ok(json!({"total": total, "list": items})), // 成功返回
        (Err(err), _) | (_, Err(err)) => db_error(err), // 失败
    }
}
async fn append_message(State(state): State<AppState>, Path(session_id): Path<String>, body: Bytes) -> ApiResult { // 追加单条消息
    let payload = body_object(&body); // 解析请求体
    let role = payload.get("role").and_then(Value::as_str).unwrap_or_default().trim().to_lowercase(); // 角色
    if !ROLES.contains(&role.as_str()) { return fail(400, "role 必须是 user/assistant"); } // 角色校验
    let text = text_of(&payload); // 消息正文
    if text.trim().is_empty() { return fail(400, "消息内容不能为空"); } // 空正文
    let message = if role == "user" { Message::user().with_text(text) } else { Message::assistant().with_text(text) }.with_generated_id_if_missing(); // 构造消息
    let conn = state.db.lock(); // 加锁读取数据库
    match sessions::get_session(&conn, &session_id) { // 校验会话
        Ok(None) => return fail(404, &format!("会话不存在: {session_id}")), // 不存在
        Err(err) => return db_error(err), // 失败
        _ => {} // 存在
    }
    match messages::upsert(&conn, &session_id, &message) { // 写入消息
        Ok(seq) => ok(json!({"item": {"id": message.id, "session_id": session_id, "seq": seq, "role": role, "message": message}})), // 返回写入项
        Err(err) => db_error(err), // 失败
    }
}
async fn delete_message_turn(State(state): State<AppState>, Path((session_id, turn_key)): Path<(String, String)>, Query(query): Query<HashMap<String, String>>) -> ApiResult { // 按轮次删除消息并同步重算会话状态与过期上下文
    let after = query.get("after").map(|text| matches!(text.trim().to_lowercase().as_str(), "1" | "true" | "yes")).unwrap_or(false); // 是否连带回滚后续轮次
    if after { state.stop(&session_id); } // 回滚前触发停止信号，避免运行中任务并发写回残余消息
    let conn = state.db.lock(); // 加锁读取数据库
    let deleted = match messages::delete_turn(&conn, &session_id, &turn_key, after) { Ok(count) => count, Err(err) => return db_error(err) }; // 从 messages 表删除
    if deleted <= 0 { return fail(404, &format!("轮次不存在: {turn_key}")); } // 未命中返回 404
    let remaining = messages::list(&conn, &session_id).unwrap_or_default(); // 检查剩余消息
    let has_hidden = remaining.iter().any(messages::is_compacted_hidden); // 是否存在被压缩隐藏的旧消息
    let has_summary = context::load_context(&conn, &session_id).ok().flatten().is_some(); // 是否已有压缩摘要
    let mut context_cleared = false; // 是否清理或重置了摘要上下文
    if remaining.is_empty() || has_summary || has_hidden { // 回滚/删轮后清理已失效摘要并恢复剩余消息可见性，同时重设累计基线
        let total_usage = usage::get_session_usage(&conn, &session_id).ok().and_then(|val| val.get("total").and_then(Value::as_i64)).unwrap_or(0); // 保留累计统计但推进基线
        let _ = messages::restore_agent_visible(&conn, &session_id); // 恢复剩余消息模型可见
        let _ = context::reset_compaction(&conn, &session_id, total_usage); // 清除失效摘要/事实并重置 80% 增量基线
        context_cleared = true; // 标记已联动刷新上下文
    }
    let status = messages::recompute_session_status(&conn, &session_id).unwrap_or("completed"); // 根据剩余消息重算会话状态
    crate::utils::logger::info("session_router", &format!("删除轮次持久化完成: session={session_id} turn={turn_key} after={after} deleted={deleted} status={status} context_cleared={context_cleared}")); // 写入日志
    ok(json!({"turn_key": turn_key, "deleted": deleted, "status": status, "context_cleared": context_cleared})) // 返回最新状态
}
async fn session_usage(State(state): State<AppState>, Path(session_id): Path<String>) -> ApiResult { // 查询会话用量
    let conn = state.db.lock(); // 加锁读取数据库
    match sessions::get_session(&conn, &session_id) { // 校验会话
        Ok(None) => return fail(404, &format!("会话不存在: {session_id}")), // 不存在
        Err(err) => return db_error(err), // 失败
        _ => {} // 存在
    }
    match usage::get_session_usage(&conn, &session_id) { // 读取用量
        Ok(data) => ok(data), // 成功返回
        Err(err) => db_error(err), // 失败
    }
}
async fn session_context(State(state): State<AppState>, Path(session_id): Path<String>) -> ApiResult { // 查询会话压缩摘要与关键事实
    let conn = state.db.lock(); // 加锁读取数据库
    match sessions::get_session(&conn, &session_id) { // 校验会话
        Ok(None) => return fail(404, &format!("会话不存在: {session_id}")), // 不存在
        Err(err) => return db_error(err), // 失败
        _ => {} // 存在
    }
    let summary = context::load_context(&conn, &session_id).unwrap_or(None); // 读取摘要
    let facts = context::load_facts(&conn, &session_id, 8).unwrap_or_default(); // 读取关键事实
    ok(json!({"summary": summary, "facts": facts})) // 返回结果
}
async fn list_turns(State(state): State<AppState>, Path(session_id): Path<String>, Query(query): Query<HashMap<String, String>>) -> ApiResult { // 分页列出轮次
    let conn = state.db.lock(); // 加锁读取数据库
    match sessions::get_session(&conn, &session_id) { // 校验会话
        Ok(None) => return fail(404, &format!("会话不存在: {session_id}")), // 不存在
        Err(err) => return db_error(err), // 失败
        _ => {} // 存在
    }
    let after_seq = query_int(&query, "after_seq", 0).max(0); // 起始序号
    let limit = query_int(&query, "limit", 50); // 分页大小
    let limit = if limit > 0 { limit } else { 50 }; // 缺省 50
    match (turns::list_turns(&conn, &session_id, after_seq, limit), turns::count_turns(&conn, &session_id)) { // 查询分页与总数
        (Ok(items), Ok(total)) => ok(json!({"total": total, "list": items})), // 成功返回
        (Err(err), _) | (_, Err(err)) => db_error(err), // 失败
    }
}
async fn append_turn(State(state): State<AppState>, Path(session_id): Path<String>, body: Bytes) -> ApiResult { // 追加轮次
    let conn = state.db.lock(); // 加锁读取数据库
    match sessions::get_session(&conn, &session_id) { // 校验会话
        Ok(None) => return fail(404, &format!("会话不存在: {session_id}")), // 不存在
        Err(err) => return db_error(err), // 失败
        _ => {} // 存在
    }
    let payload = body_object(&body); // 解析请求体
    let user = payload.get("user").filter(|value| value.is_object()).cloned().unwrap_or_else(|| json!({})); // 用户消息
    let assistant = payload.get("assistant").filter(|value| value.is_object()).cloned().unwrap_or_else(|| json!({})); // 助手消息
    if user.as_object().map(|map| map.is_empty()).unwrap_or(true) && assistant.as_object().map(|map| map.is_empty()).unwrap_or(true) { return fail(400, "user 与 assistant 不能同时为空"); } // 两者不能同时为空
    let turn_key = payload.get("turn_key").or_else(|| payload.get("turnKey")).map(|value| json_str(Some(value)).trim().to_string()).filter(|text| !text.is_empty()); // 轮次键，兼容两种命名
    match turns::append_turn(&conn, &session_id, &user, &assistant, turn_key.as_deref()) { // 幂等追加
        Ok(item) => ok(json!({"item": item})), // 成功返回
        Err(err) => db_error(err), // 失败
    }
}
async fn update_turn(State(state): State<AppState>, Path((session_id, turn_key)): Path<(String, String)>, body: Bytes) -> ApiResult { // 局部更新轮次
    let conn = state.db.lock(); // 加锁读取数据库
    match sessions::get_session(&conn, &session_id) { // 校验会话
        Ok(None) => return fail(404, &format!("会话不存在: {session_id}")), // 不存在
        Err(err) => return db_error(err), // 失败
        _ => {} // 存在
    }
    let payload = body_object(&body); // 解析请求体
    let user = payload.get("user").filter(|value| value.is_object()).cloned(); // 新用户消息
    let assistant = payload.get("assistant").filter(|value| value.is_object()).cloned(); // 新助手消息
    if user.is_none() && assistant.is_none() { return fail(400, "user 与 assistant 至少提供一个"); } // 至少提供一个
    match turns::update_turn(&conn, &session_id, &turn_key, assistant.as_ref(), user.as_ref()) { // 更新
        Ok(Some(item)) => ok(json!({"item": item})), // 成功返回
        Ok(None) => fail(404, &format!("轮次不存在: {turn_key}")), // 不存在
        Err(err) => db_error(err), // 失败
    }
}
async fn delete_turn(State(state): State<AppState>, Path((session_id, turn_key)): Path<(String, String)>, Query(query): Query<HashMap<String, String>>) -> ApiResult { // 删除轮次，支持连带后续
    let conn = state.db.lock(); // 加锁读取数据库
    match sessions::get_session(&conn, &session_id) { // 校验会话
        Ok(None) => return fail(404, &format!("会话不存在: {session_id}")), // 不存在
        Err(err) => return db_error(err), // 失败
        _ => {} // 存在
    }
    let after = query.get("after").map(|text| matches!(text.trim().to_lowercase().as_str(), "1" | "true" | "yes")).unwrap_or(false); // 是否连带删除后续
    match turns::delete_turns(&conn, &session_id, &turn_key, after) { // 删除
        Ok(deleted) if deleted > 0 => ok(json!({"turn_key": turn_key, "deleted": deleted})), // 成功返回删除数
        Ok(_) => fail(404, &format!("轮次不存在: {turn_key}")), // 不存在
        Err(err) => db_error(err), // 失败
    }
}
