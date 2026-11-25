use axum::body::Bytes; // 规则路由：/api/relus 的列举、读取、新建、保存与删除
use axum::extract::Path; // 路径参数提取
use axum::routing::get; // GET 方法
use axum::Router; // 路由器类型
use crate::agent::{rules, settings}; // 规则仓库与设置
use crate::state::{body_object, fail_exc, obj_str, ok, ApiResult, AppState}; // 请求解析与响应工具
pub fn router() -> Router<AppState> { // 注册规则路由
    Router::new() // 创建路由器
        .route("/api/relus", get(list_relus).post(create_relus)) // 列举与新建
        .route("/api/relus/{name}", get(get_relus).put(update_relus).delete(remove_relus)) // 读取/更新/删除
}
async fn list_relus() -> ApiResult { // 列出全部规则
    let settings = settings::load(); // 加载设置取规则目录
    let items = rules::list_rules(&settings.rule_dir); // 列举规则
    ok(serde_json::json!({"dir": settings.rule_dir.to_string_lossy(), "total": items.len(), "list": items})) // 返回目录、总数与列表
}
async fn get_relus(Path(name): Path<String>) -> ApiResult { // 读取单条规则
    let settings = settings::load(); // 加载设置
    match rules::read_rule(&settings.rule_dir, &name) { // 读取规则
        Ok(data) => ok(data), // 成功返回
        Err(err) => fail_exc(&err.to_string()), // 失败返回错误
    }
}
async fn create_relus(body: Bytes) -> ApiResult { // 新建规则
    let payload = body_object(&body); // 解析请求体
    let settings = settings::load(); // 加载设置
    let content = obj_str(payload.as_object().unwrap(), "content"); // 规则正文
    let trigger = payload.get("trigger").and_then(serde_json::Value::as_str).map(str::to_string); // 可选触发方式
    let description = payload.get("description").and_then(serde_json::Value::as_str).map(str::to_string); // 可选描述
    match rules::save_rule(&settings.rule_dir, &obj_str(payload.as_object().unwrap(), "name"), &content, trigger.as_deref(), description.as_deref()) { // 保存规则
        Ok(data) => ok(data), // 成功返回
        Err(err) => fail_exc(&err.to_string()), // 失败返回错误
    }
}
async fn update_relus(Path(name): Path<String>, body: Bytes) -> ApiResult { // 更新规则
    let payload = body_object(&body); // 解析请求体
    let settings = settings::load(); // 加载设置
    let content = obj_str(payload.as_object().unwrap(), "content"); // 规则正文
    let trigger = payload.get("trigger").and_then(serde_json::Value::as_str).map(str::to_string); // 可选触发方式
    let description = payload.get("description").and_then(serde_json::Value::as_str).map(str::to_string); // 可选描述
    match rules::save_rule(&settings.rule_dir, &name, &content, trigger.as_deref(), description.as_deref()) { // 覆盖保存
        Ok(data) => ok(data), // 成功返回
        Err(err) => fail_exc(&err.to_string()), // 失败返回错误
    }
}
async fn remove_relus(Path(name): Path<String>) -> ApiResult { // 删除规则
    let settings = settings::load(); // 加载设置
    match rules::delete_rule(&settings.rule_dir, &name) { // 删除规则
        Ok(data) => ok(data), // 成功返回
        Err(err) => fail_exc(&err.to_string()), // 失败返回错误
    }
}
