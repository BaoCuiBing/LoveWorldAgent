use axum::body::Bytes; // 窗口与帮助文档路由：/api/win/update 标题栏主题重绘，/api/open 打开 .loveword/doc_mds 文档
use axum::extract::State; // 状态注入
use axum::routing::{get, post}; // GET/POST 方法
use axum::Router; // 路由器类型
use serde_json::json; // JSON 构造
use crate::state::{body_object, fail, json_str, ok, ApiResult, AppState}; // 请求解析与响应工具
use crate::utils::{menu, open_file, titlebar}; // 菜单、打开文件与标题栏工具
pub fn router() -> Router<AppState> { // 注册杂项路由
    Router::new() // 创建路由器
        .route("/api/win/update", post(update_window)) // 标题栏主题更新
        .route("/api/open", post(open_document)) // 打开帮助文档
        .route("/api/data/clear", get(clear_preview).post(clear_data)) // 清除数据预览与执行
        .route("/api/app/quit", post(quit_app)) // 限时清理后台服务进程
        .route("/api/app/exit", post(exit_app)) // 退出软件窗口
}
pub async fn cleanup_services(state: &AppState) { // 后台限时 1.5s 清理终端/MCP/sidecar
    state.stop_all(); // 取消运行中对话
    let terminal = state.terminal.clone(); // 终端管理器
    let browser = state.browser.clone(); // 浏览器管理器
    let _ = tokio::time::timeout(std::time::Duration::from_millis(1500), async move {
        let mcp_task = crate::agent::mcp::shutdown_all();
        let sync_task = tokio::task::spawn_blocking(move || {
            terminal.close_all();
            browser.force_stop();
            crate::utils::computer_use::stop();
            crate::utils::office_option::force_stop();
            crate::utils::mobile::stop();
        });
        let _ = tokio::join!(mcp_task, sync_task);
    }).await;
    crate::utils::logger::info("other_router", "后台服务进程清理完成");
}
async fn quit_app(State(state): State<AppState>) -> ApiResult { // 清理全部后台服务（限时 1.5s）
    cleanup_services(&state).await;
    ok(json!({"closed": true}))
}
async fn exit_app(State(state): State<AppState>) -> ApiResult { // 响应返回后退出进程
    let handle = state.handle.lock().clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(50));
        if let Some(app) = handle { app.exit(0); }
        std::thread::sleep(std::time::Duration::from_millis(150));
        std::process::exit(0);
    });
    ok(json!({"exiting": true}))
}
fn count_table(conn: &rusqlite::Connection, table: &str) -> i64 { // 统计表行数，失败返回 0
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0)).unwrap_or(0) // 查询计数
}
fn has_running(state: &AppState, conn: &rusqlite::Connection) -> bool { // 是否有运行中会话或压缩任务
    let db_running: i64 = conn.query_row("SELECT COUNT(*) FROM sessions WHERE status = 'running'", [], |row| row.get(0)).unwrap_or(0); // 数据库 running 会话数
    db_running > 0 || !state.cancels.lock().is_empty() || state.has_active_tasks() // 数据库、取消令牌或在途任务锁任一命中
}
async fn clear_preview(State(state): State<AppState>) -> ApiResult { // 清除数据预览：返回将删除的会话数与消息数
    let guard = state.db.lock(); // 持连接锁
    let sessions = count_table(&guard, "sessions"); // 会话数
    let messages = count_table(&guard, "messages"); // 消息数
    ok(json!({"sessions": sessions, "messages": messages, "running": has_running(&state, &guard)})) // 返回预览与运行状态
}
async fn clear_data(State(state): State<AppState>) -> ApiResult { // 清除所有对话数据（六表），运行中有会话则拒绝
    let mut guard = state.db.lock(); // 持连接锁
    if has_running(&state, &guard) { return fail(409, "请先停止正在运行的任务"); } // 运行中拒绝
    let tx = match guard.transaction() { Ok(tx) => tx, Err(err) => return fail(500, &format!("开启事务失败: {err}")) }; // 开启事务
    let tables = ["messages", "turns", "session_usage", "session_context", "session_compaction", "context_facts", "sessions"]; // 七表（先子表后会话表）
    let mut counts = serde_json::Map::new(); // 各表删除计数
    for table in tables { // 逐表清空
        match tx.execute(&format!("DELETE FROM {table}"), []) { // 删除全部行
            Ok(count) => { counts.insert(table.to_string(), json!(count)); } // 记录计数
            Err(err) => return fail(500, &format!("清空 {table} 失败: {err}")), // 失败回滚并返回
        }
    }
    if let Err(err) = tx.commit() { return fail(500, &format!("提交清除事务失败: {err}")); } // 提交事务
    crate::utils::logger::info("other_router", &format!("清除对话数据完成: {counts:?}")); // 记录日志
    ok(json!(counts)) // 返回删除计数
}
async fn update_window(body: Bytes) -> ApiResult { // 更新标题栏主题与重绘策略
    let payload = body_object(&body); // 解析请求体
    let theme = payload.get("theme").and_then(serde_json::Value::as_str).unwrap_or_default().trim().to_lowercase(); // 读取主题
    if theme != "dark" && theme != "light" && theme != "system" { return fail(400, "主题必须是 dark/light/system"); } // 主题取值校验
    let repaint = payload.get("repaint").and_then(serde_json::Value::as_str).unwrap_or_default().trim().to_lowercase(); // 读取重绘策略
    if !repaint.is_empty() && !titlebar::REPAINT_MODES.contains(&repaint.as_str()) { return fail(400, &format!("重绘策略必须是 {}", titlebar::REPAINT_MODES.join("/"))); } // 重绘策略校验
    let mode = if repaint.is_empty() { None } else { Some(repaint.as_str()) }; // 空策略视为不指定
    let result = titlebar::apply(&theme, mode); // 应用标题栏主题
    menu::apply_theme(&theme); // 同步菜单主题
    crate::utils::logger::info("other_router", &format!("窗口标题栏更新: {result}")); // 记录更新日志
    ok(result) // 返回结果
}
async fn open_document(body: Bytes) -> ApiResult { // 打开帮助文档
    let payload = body_object(&body); // 解析请求体
    let name = json_str(payload.get("name")); // 文档名
    match open_file::open_doc(&name) { // 调用外部打开
        Ok(path) => ok(json!({"path": path})), // 成功返回路径
        Err(err) => { // 失败按原因分类
            let message = err.to_string(); // 错误信息
            if message.contains("不存在") { fail(404, &message) } else { fail(400, &message) } // 不存在返回 404
        }
    }
}
