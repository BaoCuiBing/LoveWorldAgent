use axum::body::Bytes; // 权限设置路由：/api/permission 读写 mode、四组逐条开关与 custom 白名单
use axum::routing::{get, post}; // GET/POST 方法
use axum::Router; // 路由器类型
use crate::agent::permission; // 权限策略模块
use crate::state::{body_object, fail, json_str, obj_str, ok, ApiResult, AppState}; // 请求解析与响应工具
pub fn router() -> Router<AppState> { // 注册权限路由
    Router::new() // 创建路由器
        .route("/api/permission", get(get_permission).put(save_permission)) // 读取/保存权限配置
        .route("/api/permission/check", post(check_permission)) // 检查命令是否需批准
        .route("/api/permission/allow", post(allow_permission)) // 加入白名单
}
async fn get_permission() -> ApiResult { // 读取权限配置快照
    ok(permission::snapshot()) // 返回快照
}
async fn save_permission(body: Bytes) -> ApiResult { // 保存权限配置
    let payload = body_object(&body); // 解析请求体
    match permission::save(&payload) { // 写入配置
        Ok(data) => ok(data), // 成功回传
        Err(err) => { // 失败记录日志
            crate::utils::logger::error("permission_router", &format!("保存权限设置失败: {err}")); // 记录错误
            fail(500, "写入配置失败") // 返回 500
        }
    }
}
async fn check_permission(body: Bytes) -> ApiResult { // 检查命令是否可直接执行
    let payload = body_object(&body); // 解析请求体
    let command = obj_str(payload.as_object().unwrap(), "command"); // 取命令文本
    if command.trim().is_empty() { return ok(serde_json::json!({"allowed": true, "reason": ""})); } // 空命令直接允许
    let allowed = !permission::needs_approval(&command); // 无需批准即允许
    ok(serde_json::json!({"allowed": allowed, "reason": if allowed { String::new() } else { permission::reason(&command) }})) // 返回结果与理由
}
async fn allow_permission(body: Bytes) -> ApiResult { // 将命令加入白名单
    let payload = body_object(&body); // 解析请求体
    let command = json_str(payload.get("command")); // 取命令文本
    if command.trim().is_empty() { return ok(serde_json::json!({"ok": false})); } // 空命令返回失败
    permission::append_custom(&command); // 追加白名单
    ok(serde_json::json!({"ok": true})) // 返回成功
}
