use axum::body::Bytes; // 移动端设置与面板状态路由：/api/mobile-option 及 /state、/steps、/stop
use axum::extract::Query; // 查询参数提取
use axum::routing::{get, post}; // GET/POST 方法
use axum::Router; // 路由器类型
use serde_json::{json, Value}; // JSON 构造
use std::collections::HashMap; // 查询参数表
use crate::agent::{agents_repo, permission, skills}; // 权限、技能与子智能体联动
use crate::config; // 可写目录
use crate::state::{body_object, fail, ok, ApiResult, AppState}; // 请求解析与响应工具
use crate::utils::mobile; // sidecar 托管
fn option_file() -> std::path::PathBuf { config::WRITABLE_MOBILE_OPTION_FILE.clone() } // 配置文件路径（agent_mobile.json）
pub fn router() -> Router<AppState> { // 注册移动端设置与面板路由
    Router::new()
        .route("/api/mobile-option", get(get_option).put(put_option)) // 读写设置
        .route("/api/mobile-option/state", get(state)) // sidecar 状态
        .route("/api/mobile-option/steps", get(steps)) // 增量操作步骤
        .route("/api/mobile-option/stop", post(stop)) // 停止 sidecar
}
async fn state() -> ApiResult { ok(mobile::snapshot()) } // 读取运行状态
async fn steps(Query(query): Query<HashMap<String, String>>) -> ApiResult { // 读取 seq 之后的步骤
    let after = query.get("after").and_then(|text| text.trim().parse::<i64>().ok()).unwrap_or(0); // 起始序号
    let (total, items) = mobile::steps(after); // 查询增量步骤
    ok(json!({"total": total, "items": items})) // 返回总数与列表
}
async fn stop() -> ApiResult { // 停止移动端 sidecar
    let _ = tokio::task::spawn_blocking(mobile::stop).await; // 清理进程
    ok(mobile::snapshot()) // 返回最新快照
}
fn read_setting(file: &Value, key: &str) -> Value { file.get(key).cloned().unwrap_or(Value::Null) } // 读取字段
async fn get_option() -> ApiResult { // 读取移动端设置
    let data = permission::load(); // 加载权限
    let enabled = data.get("groups").and_then(|groups| groups.get("mobile")).and_then(|group| group.get("enabled")).and_then(Value::as_bool).unwrap_or(true); // 组开关
    let file = config::read_json(Some(option_file().as_path())); // 共享配置
    ok(json!({"enabled": enabled, "adbPath": read_setting(&file, "adbPath"), "defaultDevice": read_setting(&file, "defaultDevice"), "screenshotMaxSize": file.get("screenshotMaxSize").cloned().unwrap_or(json!(1280)), "runtime": mobile::snapshot()})) // 返回
}
fn valid_size(value: Option<&Value>, fallback: i64) -> i64 { value.and_then(Value::as_i64).unwrap_or(fallback).clamp(320, 4096) } // 截图最大边归一化
async fn put_option(body: Bytes) -> ApiResult { // 写入移动端设置
    let payload = body_object(&body); // 解析请求体
    let Some(enabled) = payload.get("enabled").and_then(Value::as_bool) else { return fail(400, "enabled 必须为布尔值"); }; // 校验组开关
    let mut permission_data = permission::load(); // 加载权限
    if let Some(group) = permission_data.get_mut("groups").and_then(|groups| groups.get_mut("mobile")) { group["enabled"] = json!(enabled); } else { return fail(500, "权限配置缺少 mobile 组"); } // 写组开关
    let saved = match permission::save(&permission_data) { Ok(value) => value, Err(err) => return fail(500, &format!("保存移动端开关失败: {err}")) }; // 落盘权限
    let current = config::read_json(Some(option_file().as_path())); // 现有配置
    let adb_path = payload.get("adbPath").and_then(Value::as_str).map(|text| text.trim().to_string()).unwrap_or_else(|| current.get("adbPath").and_then(Value::as_str).unwrap_or("").to_string()); // adb 路径
    let default_device = payload.get("defaultDevice").and_then(Value::as_str).map(|text| text.trim().to_string()).unwrap_or_else(|| current.get("defaultDevice").and_then(Value::as_str).unwrap_or("").to_string()); // 默认设备
    let max_size = valid_size(payload.get("screenshotMaxSize").or_else(|| current.get("screenshotMaxSize")), 1280); // 截图最大边
    let config_value = json!({"adbPath": adb_path, "defaultDevice": default_device, "screenshotMaxSize": max_size}); // 组装配置
    if let Err(err) = config::atomic_write_json(&option_file(), &config_value) { return fail(500, &format!("保存移动端配置失败: {err}")); } // 落盘配置
    if !enabled { mobile::stop(); } // 关闭时结束 sidecar
    let skill_dir = config::WRITABLE_LOVEWORD_DIR.join("skills"); // 可写技能目录
    skills::sync_locked(&skill_dir, "mobile", enabled); // 同步 mobile-control 技能开关
    let _ = agents_repo::set_enabled("mobileuse-agent", enabled); // 同步 mobileuse-agent 启用状态
    let value = saved.get("groups").and_then(|groups| groups.get("mobile")).and_then(|group| group.get("enabled")).and_then(Value::as_bool).unwrap_or(enabled); // 归一化值
    ok(json!({"enabled": value, "adbPath": config_value["adbPath"], "defaultDevice": config_value["defaultDevice"], "screenshotMaxSize": max_size})) // 返回
}