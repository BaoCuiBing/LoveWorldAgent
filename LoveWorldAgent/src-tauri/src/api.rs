use axum::body::Bytes; // 桌面原生能力：目录选择写入 app.project_path、原生通知、运行环境与前端日志
use axum::extract::State; // 状态提取
use axum::routing::{get, post}; // GET/POST 方法
use axum::Router; // 路由器类型
use serde_json::json; // JSON 构造
use tauri_plugin_dialog::DialogExt; // 原生对话框
use tauri_plugin_notification::NotificationExt; // 原生通知
use crate::config; // 项目路径配置
use crate::state::{body_object, json_str, ok, ApiResult, AppState}; // 请求解析与响应工具
use crate::utils::logger; // 日志输出
pub fn router() -> Router<AppState> { // 注册桌面能力路由
    Router::new() // 创建路由器
        .route("/api/app/pick_folder", post(pick_folder)) // 选择项目目录
        .route("/api/app/notify", post(notify)) // 发送原生通知
        .route("/api/app/runtime_info", get(runtime_info)) // 运行环境信息
        .route("/api/app/log", post(write_log)) // 前端日志写入
}
fn now_text() -> String { // 当前时间字符串
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string() // 格式化
}
async fn pick_folder(State(state): State<AppState>) -> ApiResult { // 选择并保存项目目录
    let handle = state.handle.lock().clone(); // 取应用句柄
    let Some(app) = handle else { return ok(json!({"path": ""})); }; // 未就绪返回空
    let picked = tokio::task::spawn_blocking(move || app.dialog().file().set_title("选择项目文件夹").blocking_pick_folder()).await; // 阻塞式目录选择
    let path = picked.ok().flatten().and_then(|item| item.into_path().ok()).map(|item| item.to_string_lossy().to_string()).unwrap_or_default(); // 取选中路径
    if path.trim().is_empty() { return ok(json!({"path": ""})); } // 取消选择
    match config::save_project_path(&path) { // 保存路径
        Ok(resolved) => { // 成功
            logger::info("api", &format!("已选择项目目录: {}", resolved.display())); // 记录
            ok(json!({"path": resolved.to_string_lossy()})) // 返回路径
        }
        Err(err) => { // 失败
            logger::error("api", &format!("保存项目目录失败: {err}")); // 记录
            ok(json!({"path": ""})) // 返回空
        }
    }
}
async fn notify(State(state): State<AppState>, body: Bytes) -> ApiResult { // 发送原生通知
    let payload = body_object(&body); // 解析请求体
    let title = json_str(payload.get("title")); // 标题
    let text = json_str(payload.get("body")); // 正文
    let handle = state.handle.lock().clone(); // 取应用句柄
    let Some(app) = handle else { return ok(json!({"ok": false, "error": "窗口未就绪"})); }; // 未就绪
    let title = if title.trim().is_empty() { "爱满天下".to_string() } else { title }; // 缺省标题
    match app.notification().builder().title(title.clone()).body(text).show() { // 发送通知
        Ok(()) => { // 成功
            logger::info("api", &format!("原生通知已发送: {title}")); // 记录
            ok(json!({"ok": true})) // 返回成功
        }
        Err(err) => { // 失败
            logger::warn("api", &format!("原生通知失败: {err}")); // 记录
            ok(json!({"ok": false, "error": err.to_string()})) // 返回错误
        }
    }
}
async fn runtime_info() -> ApiResult { // 返回运行环境信息
    ok(json!({"tauri": tauri::VERSION, "python": "", "platform": std::env::consts::OS, "backend": "tauri-webview2", "log_file": config::LOG_DIR.join("app.log").to_string_lossy(), "time": now_text()})) // 组装信息
}
async fn write_log(body: Bytes) -> ApiResult { // 写入前端日志
    let payload = body_object(&body); // 解析请求体
    let raw = payload.get("message").and_then(serde_json::Value::as_str).unwrap_or_default().to_string(); // 日志正文
    let text = if raw.trim().is_empty() { "(空消息)".to_string() } else { raw.trim().to_string() }; // 空消息占位
    logger::info("api", &format!("前端日志: {text}")); // 写入日志
    ok(json!({"ok": true, "time": now_text(), "message": text})) // 返回结果
}
