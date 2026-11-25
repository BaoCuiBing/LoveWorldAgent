use std::collections::HashMap; // 内置浏览器路由：/api/browser/* 转交 utils/browser.rs（sidecar 启停、导航、状态、设置、步骤与 browser_* 工具）
use axum::body::Bytes; // 请求体类型
use axum::extract::{Query, State}; // 查询参数与状态提取
use axum::routing::{get, post}; // GET/POST 方法
use axum::Router; // 路由器类型
use base64::Engine; // Base64 编码
use serde_json::{json, Value}; // JSON 构造
use crate::state::{body_object, fail, json_str, ok, ApiResult, AppState}; // 请求解析与响应工具
use crate::utils::browser; // 内置浏览器客户端
use crate::utils::systeam_browser; // 系统浏览器 Cookie 读取
pub fn router() -> Router<AppState> { // 注册浏览器路由
    Router::new() // 创建路由器
        .route("/api/browser/start", post(start)) // 启动浏览器
        .route("/api/browser/rect", post(rect)) // 设置窗口位置
        .route("/api/browser/navigate", post(navigate)) // 导航
        .route("/api/browser/show", post(show)) // 显隐窗口
        .route("/api/browser/state", get(state)) // 浏览器状态
        .route("/api/browser/stop", post(stop)) // 停止浏览器
        .route("/api/browser/tool", post(tool)) // 执行 browser_* 工具
        .route("/api/browser/settings", get(get_settings).put(save_settings)) // 设置读写
        .route("/api/browser/steps", get(steps)) // 步骤列表
        .route("/api/browser/screenshot", get(screenshot)) // 截图读取
        .route("/api/browser/screenshot/clear", post(clear_screenshots)) // 清空截图
        .route("/api/browser/import/sources", get(import_sources)) // 可导入来源
        .route("/api/browser/import/browser", post(import_browser)) // 从系统浏览器导入
}
fn rect_of(payload: &Value) -> (f64, f64, f64, f64) { // 从请求提取窗口矩形
    let raw = payload.get("rect").cloned().unwrap_or_else(|| json!({})); // rect 对象
    let value = |key: &str| raw.get(key).and_then(Value::as_f64).unwrap_or(0.0); // 取数值字段闭包
    (value("x"), value("y"), value("w"), value("h")) // 返回四元组
}
async fn start(State(state): State<AppState>, body: Bytes) -> ApiResult { // 启动内置浏览器
    let payload = body_object(&body); // 解析请求体
    let url = json_str(payload.get("url")); // 初始网址
    let manager = state.browser.clone(); // 克隆浏览器管理器
    let result = manager.start_async(rect_of(&payload), &url); // 异步启动
    ok(result) // 返回结果
}
async fn rect(State(state): State<AppState>, body: Bytes) -> ApiResult { // 设置浏览器窗口位置
    let payload = body_object(&body); // 解析请求体
    ok(state.browser.set_rect(rect_of(&payload))) // 设置矩形
}
async fn navigate(State(state): State<AppState>, body: Bytes) -> ApiResult { // 导航到指定网址
    let payload = body_object(&body); // 解析请求体
    let url = json_str(payload.get("url")); // 目标网址
    let manager = state.browser.clone(); // 克隆管理器
    match tokio::task::spawn_blocking(move || manager.navigate(&url)).await { // 阻塞操作丢到线程池
        Ok(result) => ok(result), // 成功返回
        Err(err) => fail(500, &format!("导航失败：{err}")), // 失败返回错误
    }
}
async fn show(State(state): State<AppState>, body: Bytes) -> ApiResult { // 显隐浏览器窗口
    let payload = body_object(&body); // 解析请求体
    ok(state.browser.show(payload.get("value").and_then(Value::as_bool).unwrap_or(false))) // 设置显隐
}
async fn state(State(state): State<AppState>) -> ApiResult { // 读取浏览器状态
    ok(state.browser.snapshot()) // 返回快照
}
async fn stop(State(state): State<AppState>) -> ApiResult { // 停止浏览器
    let manager = state.browser.clone(); // 克隆管理器
    match tokio::task::spawn_blocking(move || manager.stop()).await { // 阻塞操作丢到线程池
        Ok(result) => ok(result), // 成功返回
        Err(err) => fail(500, &format!("停止浏览器失败：{err}")), // 失败返回错误
    }
}
async fn tool(State(state): State<AppState>, body: Bytes) -> ApiResult { // 执行 browser_*/ui_* 工具
    let payload = body_object(&body); // 解析请求体
    let name = json_str(payload.get("name")); // 工具名
    if !(name.starts_with("browser_") || name.starts_with("ui_")) { return ok(json!({"ok": false, "error": "name 需以 browser_ 或 ui_ 开头"})); } // 名称前缀校验
    let args = payload.get("args").filter(|value| value.is_object()).cloned().unwrap_or_else(|| json!({})); // 工具参数
    let timeout = payload.get("timeout").and_then(Value::as_f64).unwrap_or(browser::DEFAULT_TIMEOUT); // 超时秒数
    let manager = state.browser.clone(); // 克隆管理器
    match tokio::task::spawn_blocking(move || manager.tool(&name, &args, timeout)).await { // 阻塞操作丢到线程池
        Ok(result) => ok(result), // 成功返回
        Err(err) => fail(500, &format!("浏览器指令失败：{err}")), // 失败返回错误
    }
}
async fn get_settings(State(state): State<AppState>) -> ApiResult { // 读取浏览器设置
    ok(state.browser.settings()) // 返回设置
}
async fn save_settings(State(state): State<AppState>, body: Bytes) -> ApiResult { // 保存浏览器设置
    let payload = body_object(&body); // 解析请求体
    let enabled_change = payload.get("enabled").and_then(serde_json::Value::as_bool); // 读取总开关变更
    let manager = state.browser.clone(); // 克隆管理器
    match tokio::task::spawn_blocking(move || manager.save_settings(&payload)).await { // 阻塞操作丢到线程池
        Ok(result) => { // 成功
            if let Some(enabled) = enabled_change { // 开关变更时同步技能与子智能体
                let skill_dir = crate::config::WRITABLE_LOVEWORD_DIR.join("skills"); // 可写技能目录
                crate::agent::skills::sync_locked(&skill_dir, "browser", enabled); // 同步 browser-automation
                let _ = crate::agent::agents_repo::set_enabled("browser-agent", enabled); // 同步 browser-agent
            }
            ok(result) // 返回设置
        }
        Err(err) => fail(500, &format!("保存浏览器设置失败：{err}")), // 失败返回错误
    }
}
async fn steps(State(state): State<AppState>, Query(query): Query<HashMap<String, String>>) -> ApiResult { // 列出 seq 之后的步骤
    let after = query.get("after").and_then(|text| text.trim().parse::<i64>().ok()).unwrap_or(0); // 起始序号
    let all = state.browser.steps(); // 全部步骤
    let items: Vec<Value> = all.iter().filter(|item| item.get("seq").and_then(Value::as_i64).unwrap_or(0) > after).cloned().collect(); // 过滤增量
    ok(json!({"total": all.len(), "items": items})) // 返回总数与增量
}
async fn screenshot(Query(query): Query<HashMap<String, String>>) -> ApiResult { // 读取截图并 Base64 编码
    let raw = query.get("path").cloned().unwrap_or_default(); // 截图路径
    let root = std::fs::canonicalize(browser::screenshot_dir()).unwrap_or_else(|_| browser::screenshot_dir()); // 截图根目录
    let path = std::path::PathBuf::from(&raw); // 目标路径
    let resolved = std::fs::canonicalize(&path).ok(); // 规范化路径
    let inside = resolved.as_ref().map(|item| item.starts_with(&root)).unwrap_or(false); // 是否越界
    let Some(target) = resolved.filter(|_| inside && path.is_absolute()) else { return fail(404, "截图不存在"); }; // 越界或不存在
    let Ok(bytes) = std::fs::read(&target) else { return fail(404, "截图不存在"); }; // 读取文件
    let mime = match target.extension().map(|item| item.to_string_lossy().to_lowercase()).unwrap_or_default().as_str() { // 按扩展名定 MIME
        "jpg" | "jpeg" => "image/jpeg", // JPEG
        _ => "image/png", // 默认 PNG
    };
    ok(json!({"path": target.to_string_lossy(), "mime": mime, "base64": base64::engine::general_purpose::STANDARD.encode(bytes)})) // 返回结果
}
async fn clear_screenshots(State(state): State<AppState>) -> ApiResult { // 清空截图
    let result = state.browser.clear_screenshots(); // 执行清理
    ok(json!({"deleted": result.get("data").and_then(|item| item.get("deleted")).and_then(Value::as_u64).unwrap_or(0)})) // 返回删除数量
}
async fn import_sources() -> ApiResult { // 列出可导入来源
    ok(systeam_browser::list_sources()) // 返回来源列表
}
async fn import_browser(State(state): State<AppState>, body: Bytes) -> ApiResult { // 从系统浏览器导入 Cookie
    let payload = body_object(&body); // 解析请求体
    let source = json_str(payload.get("source")); // 来源名称
    let include_expired = payload.get("include_expired").and_then(Value::as_bool).unwrap_or(false); // 是否含过期项
    let manager = state.browser.clone(); // 克隆管理器
    let target = source.clone(); // 克隆来源名
    let result = tokio::task::spawn_blocking(move || { // 阻塞操作丢到线程池
        let data = systeam_browser::read_cookies(&target, include_expired)?; // 读取 Cookie
        let cookies = data.get("cookies").and_then(Value::as_array).cloned().unwrap_or_default(); // 取 Cookie 数组
        let mut imported = 0usize; // 导入计数
        for chunk in cookies.chunks(100) { // 每 100 条一批
            let payload = json!({"cookies": chunk}); // 批次载荷
            if manager.import_payload(&payload).get("ok").and_then(Value::as_bool).unwrap_or(false) { imported += chunk.len(); } // 成功则累加
        }
        Ok::<(Value, usize, usize), String>((data, cookies.len(), imported)) // 返回结果三元组
    }).await;
    match result { // 处理结果
        Ok(Ok((data, total, imported))) => { // 导入成功
            let errors: Vec<Value> = data.get("errors").and_then(Value::as_array).cloned().unwrap_or_default().iter().map(|item| json!({"profile": item.get("profile").cloned().unwrap_or(Value::Null), "reason": item.get("reason").cloned().unwrap_or(Value::Null)})).collect(); // 汇总错误信息
            ok(json!({"source": source, "total": total, "imported": imported, "skipped": total - imported, "reasons": data.get("skipped").cloned().unwrap_or_else(|| json!({})), "errors": errors})) // 返回导入统计
        }
        Ok(Err(message)) => fail(400, &message), // 读取失败
        Err(err) => fail(500, &format!("导入 Cookie 失败：{err}")), // 线程错误
    }
}
