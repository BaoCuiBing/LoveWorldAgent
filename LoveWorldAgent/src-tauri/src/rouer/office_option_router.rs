use axum::body::Bytes; // Office 工具设置、独立自动截图开关、截图读取与面板状态路由
use axum::extract::Query; // 查询参数提取
use axum::routing::{get, post}; // GET/POST 方法
use axum::Router; // 路由器类型
use serde_json::{json, Value}; // JSON 构造
use std::collections::HashMap; // 查询参数表
use crate::agent::{agents_repo, permission, skills}; // 权限、技能与子智能体联动
use crate::config; // 可写目录
use crate::state::{body_object, fail, ok, ApiResult, AppState}; // 请求解析与响应工具
use crate::utils::{office_option, sidecar_pipe}; // sidecar 托管与通用截图读取助手

pub fn router() -> Router<AppState> { // 注册三族设置、截图读取与面板路由
    Router::new() // 创建路由器
        .route("/api/word-option", get(get_word).put(put_word)) // Word
        .route("/api/ppt-option", get(get_ppt).put(put_ppt)) // PPT
        .route("/api/excel-option", get(get_excel).put(put_excel)) // Excel
        .route("/api/office-option/state", get(state)) // sidecar 状态
        .route("/api/office-option/steps", get(steps)) // 增量操作步骤
        .route("/api/office-option/screenshot", get(screenshot)) // 截图读取
        .route("/api/office-option/screenshot/clear", post(clear_screenshots)) // 清空截图
        .route("/api/office-option/stop", post(stop)) // 停止 sidecar
}
async fn state() -> ApiResult { ok(office_option::snapshot()) } // 读取运行状态
async fn steps(Query(query): Query<HashMap<String, String>>) -> ApiResult { // 读取 seq 之后的步骤
    let after = query.get("after").and_then(|text| text.trim().parse::<i64>().ok()).unwrap_or(0); // 起始序号
    let (total, items) = office_option::steps(after); // 查询增量步骤
    ok(json!({"total": total, "items": items})) // 返回总数与列表
}
async fn screenshot(Query(query): Query<HashMap<String, String>>) -> ApiResult { // 读取 Office 窗口截图 Base64
    let raw = query.get("path").cloned().unwrap_or_default(); // 截图路径
    match sidecar_pipe::read_screenshot_base64(&office_option::screenshot_dir(), &raw) {
        Some(payload) => ok(payload),
        None => fail(404, "截图不存在"),
    }
}
async fn clear_screenshots() -> ApiResult { // 清空 Office 截图目录
    let deleted = tokio::task::spawn_blocking(office_option::clear_screenshots).await.unwrap_or(0);
    ok(json!({"deleted": deleted}))
}
async fn stop() -> ApiResult { // 停止 Office sidecar
    let _ = tokio::task::spawn_blocking(office_option::stop).await; // 清理进程
    ok(office_option::snapshot()) // 返回最新快照
}
async fn get_word() -> ApiResult { read_config("word") } // 读取 Word 配置
async fn put_word(body: Bytes) -> ApiResult { write_config("word", &body) } // 写入 Word 配置
async fn get_ppt() -> ApiResult { read_config("ppt") } // 读取 PPT 配置
async fn put_ppt(body: Bytes) -> ApiResult { write_config("ppt", &body) } // 写入 PPT 配置
async fn get_excel() -> ApiResult { read_config("excel") } // 读取 Excel 配置
async fn put_excel(body: Bytes) -> ApiResult { write_config("excel", &body) } // 写入 Excel 配置

fn read_config(family: &str) -> ApiResult { // 读取指定族开关、共享备份数与独立自动截图开关（不再含 allowDirs）
    let data = permission::load(); // 加载权限
    let enabled = data.get("groups").and_then(|groups| groups.get(family)).and_then(|group| group.get("enabled")).and_then(Value::as_bool).unwrap_or(true); // 组开关
    let backup_keep = office_option::backup_keep(); // 备份保留数
    let auto_screenshot = office_option::auto_screenshot_enabled(family); // 该族自动截图开关
    ok(json!({"enabled": enabled, "backupKeep": backup_keep, "autoScreenshot": auto_screenshot, "runtime": office_option::snapshot()})) // 返回
}

fn write_config(family: &str, body: &[u8]) -> ApiResult { // 独立写入族开关、备份保留数或当前族自动截图开关
    let payload = body_object(body); // 解析请求体
    let enabled_opt = match payload.get("enabled") {
        None => None,
        Some(value) => match value.as_bool() { Some(flag) => Some(flag), None => return fail(400, "enabled 必须为布尔值") },
    };
    let backup_opt = match payload.get("backupKeep").or_else(|| payload.get("backup_keep")) {
        None => None,
        Some(value) => match value.as_i64() { Some(num) => Some(num), None => return fail(400, "backupKeep 必须为整数") },
    };
    let auto_raw = payload.get("autoScreenshot").or_else(|| payload.get("auto_screenshot"));
    let auto_opt = match auto_raw {
        None => None,
        Some(value) => match value.as_bool() { Some(flag) => Some(flag), None => return fail(400, "autoScreenshot 必须为布尔值") },
    };
    if enabled_opt.is_none() && backup_opt.is_none() && auto_opt.is_none() { return fail(400, "缺少 enabled、backupKeep 或 autoScreenshot 参数"); }
    let mut final_enabled = permission::load().get("groups").and_then(|groups| groups.get(family)).and_then(|group| group.get("enabled")).and_then(Value::as_bool).unwrap_or(true);
    if let Some(enabled) = enabled_opt {
        let mut permission_data = permission::load(); // 加载权限
        if let Some(group) = permission_data.get_mut("groups").and_then(|groups| groups.get_mut(family)) { group["enabled"] = json!(enabled); } else { return fail(500, &format!("权限配置缺少 {family} 组")); } // 写组开关
        let saved = match permission::save(&permission_data) { Ok(value) => value, Err(err) => return fail(500, &format!("保存 {family} 开关失败: {err}")) }; // 落盘权限
        office_option::set_enabled(family, enabled); // 同步 sidecar 族开关
        let skill_dir = config::WRITABLE_LOVEWORD_DIR.join("skills"); // 可写技能目录
        skills::sync_locked(&skill_dir, family, enabled); // 同步锁定技能
        let agent_id = match family { "ppt" => "pptuse-agent", "excel" => "exceluse-agent", _ => "worduse-agent" }; // 内置智能体
        let _ = agents_repo::set_enabled(agent_id, enabled); // 同步内置智能体
        final_enabled = saved.get("groups").and_then(|groups| groups.get(family)).and_then(|group| group.get("enabled")).and_then(Value::as_bool).unwrap_or(enabled); // 归一化值
    }
    let (saved_keep, saved_auto) = match office_option::save_office_config(family, backup_opt, auto_opt) {
        Ok(res) => res,
        Err(err) => return fail(500, &format!("保存 Office 配置失败: {err}")),
    };
    ok(json!({"enabled": final_enabled, "backupKeep": saved_keep, "autoScreenshot": saved_auto})) // 返回最新状态
}
#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    async fn decode_api(res: ApiResult) -> (StatusCode, Value) {
        let response = res.into_response();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        (status, body)
    }
    #[tokio::test]
    async fn office_options_remove_allow_dirs_and_persist_auto_screenshot_independently() {
        let (_, w0) = decode_api(get_word().await).await;
        let (_, p0) = decode_api(get_ppt().await).await;
        let (_, e0) = decode_api(get_excel().await).await;
        assert!(w0["data"].get("allowDirs").is_none());
        assert!(p0["data"].get("allowDirs").is_none());
        assert!(e0["data"].get("allowDirs").is_none());
        let orig_w = w0["data"]["autoScreenshot"].as_bool().unwrap();
        let orig_p = p0["data"]["autoScreenshot"].as_bool().unwrap();
        let (bad_s, _) = decode_api(put_word(Bytes::from_static(b"{}")).await).await;
        assert_eq!(bad_s, StatusCode::BAD_REQUEST);
        let (bad_auto, _) = decode_api(put_ppt(Bytes::from_static(br#"{"autoScreenshot":"bad"}"#)).await).await;
        assert_eq!(bad_auto, StatusCode::BAD_REQUEST);
        let (_, w1) = decode_api(put_word(Bytes::from(json!({"autoScreenshot": !orig_w}).to_string())).await).await;
        assert_eq!(w1["data"]["autoScreenshot"], json!(!orig_w));
        let (_, p_check) = decode_api(get_ppt().await).await;
        assert_eq!(p_check["data"]["autoScreenshot"], json!(orig_p));
        let _ = put_word(Bytes::from(json!({"autoScreenshot": orig_w}).to_string())).await;
    }
    #[tokio::test]
    async fn office_screenshot_blocks_traversal_and_reads_file_inside_office_screen() {
        let outside = std::env::temp_dir().join("outside_office_secret.png");
        std::fs::write(&outside, b"no").unwrap();
        let mut q_out = HashMap::new();
        q_out.insert("path".to_string(), outside.to_string_lossy().to_string());
        let (s_out, _) = decode_api(screenshot(Query(q_out)).await).await;
        assert_eq!(s_out, StatusCode::NOT_FOUND);
        let _ = std::fs::remove_file(&outside);
        let inside = office_option::screenshot_dir().join(format!("test_shot_{}.png", std::process::id()));
        std::fs::write(&inside, b"pngdata").unwrap();
        let mut q_in = HashMap::new();
        q_in.insert("path".to_string(), inside.to_string_lossy().to_string());
        let (s_in, body_in) = decode_api(screenshot(Query(q_in)).await).await;
        assert_eq!(s_in, StatusCode::OK);
        assert_eq!(body_in["data"]["mime"], json!("image/png"));
        assert!(body_in["data"]["base64"].as_str().map(|s| !s.is_empty()).unwrap_or(false));
        let _ = std::fs::remove_file(&inside);
    }
}
