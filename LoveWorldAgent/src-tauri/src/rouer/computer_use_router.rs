use axum::body::Bytes; // 电脑控制总开关、自动截图设置、截图读取与面板状态路由
use axum::extract::Query; // 查询参数提取
use axum::routing::{get, post}; // GET/POST 方法
use axum::Router; // 路由器类型
use serde_json::{json, Value}; // JSON 构造
use std::collections::HashMap; // 查询参数表
use crate::agent::{agents_repo, permission, skills}; // 权限组、技能与子智能体联动
use crate::config; // 可写目录
use crate::state::{body_object, fail, ok, ApiResult, AppState}; // 请求解析与响应工具
use crate::utils::{computer_use, sidecar_pipe}; // sidecar 托管与通用截图助手
pub fn router() -> Router<AppState> { // 注册电脑控制路由
    Router::new() // 创建路由器
        .route("/api/computer-use", get(get_computer_use).put(put_computer_use)) // 读写开关与自动截图设置
        .route("/api/computer-use/state", get(state)) // sidecar 状态
        .route("/api/computer-use/steps", get(steps)) // 增量操作步骤
        .route("/api/computer-use/stop", post(stop)) // 停止 sidecar
        .route("/api/computer-use/screenshot", get(screenshot)) // 截图读取
        .route("/api/computer-use/screenshot/clear", post(clear_screenshots)) // 清空截图
}
async fn get_computer_use() -> ApiResult { // 读取 computer 组启用状态、自动截图开关与运行态
    let data = permission::load(); // 加载权限配置
    let enabled = data.get("groups").and_then(|groups| groups.get("computer")).and_then(|group| group.get("enabled")).and_then(Value::as_bool).unwrap_or(true); // 缺省启用
    ok(json!({"ok": true, "enabled": enabled, "auto_screenshot": computer_use::auto_screenshot_enabled(), "runtime": computer_use::snapshot()})) // 返回开关与运行态
}
async fn state() -> ApiResult { ok(computer_use::snapshot()) } // 读取运行状态
async fn steps(Query(query): Query<HashMap<String, String>>) -> ApiResult { // 读取 seq 之后的步骤
    let after = query.get("after").and_then(|text| text.trim().parse::<i64>().ok()).unwrap_or(0); // 起始序号
    let (total, items) = computer_use::steps(after); // 查询增量步骤
    ok(json!({"total": total, "items": items})) // 返回总数与列表
}
async fn stop() -> ApiResult { // 停止电脑控制 sidecar
    let _ = tokio::task::spawn_blocking(computer_use::stop).await; // 清理进程
    ok(computer_use::snapshot()) // 返回最新快照
}
async fn put_computer_use(body: Bytes) -> ApiResult { // 独立写入 computer 组启用状态与自动截图开关
    let payload = body_object(&body); // 解析请求体
    let enabled_opt = match payload.get("enabled") {
        None => None,
        Some(value) => match value.as_bool() {
            Some(flag) => Some(flag),
            None => return fail(400, "enabled 必须为布尔值"),
        },
    };
    let auto_opt = match payload.get("auto_screenshot") {
        None => None,
        Some(value) => match value.as_bool() {
            Some(flag) => Some(flag),
            None => return fail(400, "auto_screenshot 必须为布尔值"),
        },
    };
    if enabled_opt.is_none() && auto_opt.is_none() { return fail(400, "缺少 enabled 或 auto_screenshot 参数"); }
    let mut final_enabled = permission::load().get("groups").and_then(|groups| groups.get("computer")).and_then(|group| group.get("enabled")).and_then(Value::as_bool).unwrap_or(true);
    if let Some(enabled) = enabled_opt {
        let mut data = permission::load(); // 加载当前配置
        if let Some(group) = data.get_mut("groups").and_then(|groups| groups.get_mut("computer")) { group["enabled"] = json!(enabled); } else { return fail(500, "权限配置缺少 computer 组"); } // 写入组开关
        let saved = match permission::save(&data) { Ok(value) => value, Err(err) => return fail(500, &format!("保存电脑控制开关失败: {err}")) }; // 落盘
        if !enabled { computer_use::stop(); } // 关闭时结束 sidecar 与管道
        let skill_dir = config::WRITABLE_LOVEWORD_DIR.join("skills"); // 可写技能目录
        skills::sync_locked(&skill_dir, "computer", enabled); // 同步 computer-control 技能开关
        let _ = agents_repo::set_enabled("computeruse-agent", enabled); // 同步 computeruse-agent 启用状态
        final_enabled = saved.get("groups").and_then(|groups| groups.get("computer")).and_then(|group| group.get("enabled")).and_then(Value::as_bool).unwrap_or(enabled); // 读取归一化后的值
    }
    if let Some(auto_shot) = auto_opt {
        if let Err(err) = computer_use::save_auto_screenshot(auto_shot) {
            return fail(500, &format!("保存电脑控制自动截图开关失败: {err}"));
        }
    }
    ok(json!({"ok": true, "enabled": final_enabled, "auto_screenshot": computer_use::auto_screenshot_enabled()})) // 返回独立持久化后的结果
}
async fn screenshot(Query(query): Query<HashMap<String, String>>) -> ApiResult { // 读取截图并 Base64 编码
    let raw = query.get("path").cloned().unwrap_or_default(); // 截图路径
    match sidecar_pipe::read_screenshot_base64(&computer_use::screenshot_dir(), &raw) {
        Some(payload) => ok(payload),
        None => fail(404, "截图不存在"),
    }
}
async fn clear_screenshots() -> ApiResult { // 清空电脑控制截图目录
    let deleted = tokio::task::spawn_blocking(computer_use::clear_screenshots).await.unwrap_or(0);
    ok(json!({"deleted": deleted}))
}
#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use base64::Engine;
    async fn decode_api(res: ApiResult) -> (StatusCode, Value) {
        let response = res.into_response();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        (status, body)
    }
    #[tokio::test]
    async fn get_and_put_computer_use_validate_and_persist_independently() {
        let (status, body) = decode_api(get_computer_use().await).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["data"]["ok"], json!(true));
        assert!(body["data"]["enabled"].is_boolean());
        assert!(body["data"]["auto_screenshot"].is_boolean());
        let original_auto = body["data"]["auto_screenshot"].as_bool().unwrap();
        let (bad_empty_status, _) = decode_api(put_computer_use(Bytes::from_static(b"{}")).await).await;
        assert_eq!(bad_empty_status, StatusCode::BAD_REQUEST);
        let (bad_enabled_status, _) = decode_api(put_computer_use(Bytes::from_static(br#"{"enabled":"yes"}"#)).await).await;
        assert_eq!(bad_enabled_status, StatusCode::BAD_REQUEST);
        let (bad_auto_status, _) = decode_api(put_computer_use(Bytes::from_static(br#"{"auto_screenshot":"yes"}"#)).await).await;
        assert_eq!(bad_auto_status, StatusCode::BAD_REQUEST);
        let toggled = !original_auto;
        let payload = json!({"auto_screenshot": toggled}).to_string();
        let (put_status, put_body) = decode_api(put_computer_use(Bytes::from(payload)).await).await;
        assert_eq!(put_status, StatusCode::OK);
        assert_eq!(put_body["data"]["auto_screenshot"], json!(toggled));
        let restore_payload = json!({"auto_screenshot": original_auto}).to_string();
        let (restore_status, restore_body) = decode_api(put_computer_use(Bytes::from(restore_payload)).await).await;
        assert_eq!(restore_status, StatusCode::OK);
        assert_eq!(restore_body["data"]["auto_screenshot"], json!(original_auto));
    }
    #[tokio::test]
    async fn screenshot_and_clear_endpoints_enforce_boundary_and_serve_png() {
        let mut bad_query = HashMap::new();
        bad_query.insert("path".to_string(), "C:\\Windows\\System32\\drivers\\etc\\hosts".to_string());
        let (miss_status, _) = decode_api(screenshot(Query(bad_query)).await).await;
        assert_eq!(miss_status, StatusCode::NOT_FOUND);
        let temp_shot = computer_use::screenshot_dir().join(format!("_test_cu_shot_{}.png", std::process::id()));
        std::fs::write(&temp_shot, b"fake-png-bytes").unwrap();
        let mut ok_query = HashMap::new();
        ok_query.insert("path".to_string(), temp_shot.to_string_lossy().to_string());
        let (ok_status, ok_body) = decode_api(screenshot(Query(ok_query)).await).await;
        assert_eq!(ok_status, StatusCode::OK);
        assert_eq!(ok_body["data"]["mime"], json!("image/png"));
        assert_eq!(ok_body["data"]["base64"], json!(base64::engine::general_purpose::STANDARD.encode(b"fake-png-bytes")));
        let (clear_status, clear_body) = decode_api(clear_screenshots().await).await;
        let _ = std::fs::remove_file(&temp_shot);
        assert_eq!(clear_status, StatusCode::OK);
        assert!(clear_body["data"]["deleted"].as_u64().unwrap_or(0) >= 1);
    }
}
