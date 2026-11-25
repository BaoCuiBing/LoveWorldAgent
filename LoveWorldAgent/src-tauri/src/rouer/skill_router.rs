use std::collections::HashMap; // 技能路由：/api/skills 的列举、读取、新建、保存、删除与启停
use axum::body::Bytes; // 请求体类型
use axum::extract::{Path, Query}; // 路径与查询参数
use axum::routing::get; // GET 方法
use axum::Router; // 路由器类型
use crate::agent::{settings, skills}; // 设置与技能仓库
use crate::state::{as_bool, body_object, fail_exc, json_str, obj_str, ok, ApiResult, AppState}; // 请求解析与响应工具
pub fn router() -> Router<AppState> { // 注册技能路由
    Router::new() // 创建路由器
        .route("/api/skills", get(list_skills).post(create_skill)) // 列举与新建
        .route("/api/skills/{name}", get(get_skill).put(update_skill).delete(remove_skill)) // 读取/更新/删除
        .route("/api/skills/{name}/enabled", axum::routing::patch(toggle_skill)) // 启停切换
}
async fn list_skills(Query(query): Query<HashMap<String, String>>) -> ApiResult { // 列举技能，支持关键字
    let settings = settings::load(); // 加载设置取技能目录
    let keyword = query.get("q").cloned().unwrap_or_default(); // 查询关键字
    let items = skills::list_skills(&settings.skill_dir, &keyword); // 列举技能
    ok(serde_json::json!({"dir": settings.skill_dir.to_string_lossy(), "total": items.len(), "list": items})) // 返回目录、总数与列表
}
async fn get_skill(Path(name): Path<String>) -> ApiResult { // 读取单个技能
    let settings = settings::load(); // 加载设置
    match skills::read_skill(&settings.skill_dir, &name) { // 读取技能
        Ok(data) => ok(data), // 成功返回
        Err(err) => fail_exc(&err.to_string()), // 失败返回错误
    }
}
async fn create_skill(body: Bytes) -> ApiResult { // 新建技能
    let payload = body_object(&body); // 解析请求体
    let map = payload.as_object().unwrap(); // 对象引用
    let settings = settings::load(); // 加载设置
    let enabled = as_bool(payload.get("enabled"), true); // 缺省启用
    match skills::save_skill(&settings.skill_dir, &obj_str(map, "name"), &obj_str(map, "description"), &obj_str(map, "content"), enabled) { // 保存技能
        Ok(data) => ok(data), // 成功返回
        Err(err) => fail_exc(&err.to_string()), // 失败返回错误
    }
}
async fn update_skill(Path(name): Path<String>, body: Bytes) -> ApiResult { // 更新技能，未提交字段保留现值
    let payload = body_object(&body); // 解析请求体
    let settings = settings::load(); // 加载设置
    let current = match skills::read_skill(&settings.skill_dir, &name) { Ok(data) => data, Err(err) => return fail_exc(&err.to_string()) }; // 读取现值
    let description = payload.get("description").map(|value| json_str(Some(value))).unwrap_or_else(|| current.get("description").and_then(serde_json::Value::as_str).unwrap_or_default().to_string()); // 描述缺省用现值
    let content = payload.get("content").map(|value| json_str(Some(value))).unwrap_or_else(|| current.get("body").and_then(serde_json::Value::as_str).unwrap_or_default().to_string()); // 正文缺省用现值
    let enabled = as_bool(payload.get("enabled"), current.get("enabled").and_then(serde_json::Value::as_bool).unwrap_or(true)); // 启用状态缺省用现值
    match skills::save_skill(&settings.skill_dir, &name, &description, &content, enabled) { // 覆盖保存
        Ok(data) => ok(data), // 成功返回
        Err(err) => fail_exc(&err.to_string()), // 失败返回错误
    }
}
async fn remove_skill(Path(name): Path<String>) -> ApiResult { // 删除技能
    let settings = settings::load(); // 加载设置
    match skills::delete_skill(&settings.skill_dir, &name) { // 删除技能
        Ok(data) => ok(data), // 成功返回
        Err(err) => fail_exc(&err.to_string()), // 失败返回错误
    }
}
async fn toggle_skill(Path(name): Path<String>, body: Bytes) -> ApiResult { // 切换技能启用状态
    let payload = body_object(&body); // 解析请求体
    let settings = settings::load(); // 加载设置
    match skills::set_enabled(&settings.skill_dir, &name, as_bool(payload.get("enabled"), true)) { // 写入启用状态
        Ok(data) => ok(data), // 成功返回
        Err(err) => fail_exc(&err.to_string()), // 失败返回错误
    }
}
