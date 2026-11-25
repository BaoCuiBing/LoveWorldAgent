use std::collections::HashMap; // MCP 配置路由：/api/mcp 读写 servers 数组，支持表单 CRUD 与整文件 JSON 读写，env/headers 脱敏
use axum::body::Bytes; // 请求体类型
use axum::extract::{Path, Query}; // 路径与查询参数
use axum::routing::{get, patch}; // GET/PATCH 方法
use axum::Router; // 路由器类型
use serde_json::{json, Map, Value}; // JSON 构造
use crate::agent::mcp as mcp_store; // MCP 配置与状态模块
use crate::state::{body_object, fail, obj_str, ok, ApiResult, AppState}; // 请求解析与响应工具
pub fn router() -> Router<AppState> { // 注册 MCP 配置路由
    Router::new() // 创建路由器
        .route("/api/mcp", get(list_mcp).post(create_mcp)) // 列举与新建
        .route("/api/mcp/status", get(mcp_status)) // 连接状态查询
        .route("/api/mcp/json", get(get_mcp_json).put(put_mcp_json)) // 整文件 JSON 读写
        .route("/api/mcp/{name}", axum::routing::put(update_mcp).delete(delete_mcp)) // 更新/删除
        .route("/api/mcp/{name}/enabled", patch(toggle_mcp)) // 启停切换
}
fn public_list() -> Vec<Value> { // 返回脱敏后的服务列表
    mcp_store::load_servers().iter().map(mcp_store::public).collect() // 逐服务脱敏
}
fn find_index(name: &str) -> Option<usize> { // 按名称查找服务下标
    mcp_store::load_servers().iter().position(|item| mcp_store::normalize(item).get("name").and_then(Value::as_str) == Some(name)) // 归一化后比较名称
}
fn validate(payload: &Value, existing: Option<&Value>) -> (Option<Value>, String) { // 合并并校验服务配置
    let merged = mcp_store::merge_item(existing, payload); // 合并提交与原值
    mcp_store::validate(&merged) // 委托校验
}
fn unique_check(data: &Value, items: &[Value], exclude: &str) -> bool { // 检查名称是否与其它服务重复
    let candidate = data.get("name").and_then(Value::as_str).unwrap_or_default(); // 目标名称
    items.iter().filter(|item| mcp_store::normalize(item).get("name").and_then(Value::as_str) != Some(exclude)).any(|item| mcp_store::normalize(item).get("name").and_then(Value::as_str) == Some(candidate)) // 排除自身后重名检测
}
fn list_response(items: &[Value]) -> ApiResult { // 构造列表响应
    ok(json!({"total": items.len(), "list": public_list()})) // 返回总数与脱敏列表
}
async fn list_mcp() -> ApiResult { // 列出全部 MCP 服务
    let items = public_list(); // 脱敏列表
    ok(json!({"total": items.len(), "list": items, "types": mcp_store::TYPES})) // 附带支持的传输类型
}
fn status_name(item: &Value) -> String { // 取服务名用于状态匹配
    item.get("name").and_then(Value::as_str).unwrap_or_default().to_string() // 字符串化
}
fn status_default(item: &Value) -> Value { // 生成默认状态
    if item.get("enabled").and_then(Value::as_bool).unwrap_or(true) { json!({"status": "connecting", "tools": 0, "error": "", "updated_at": 0}) } else { json!({"status": "disabled", "tools": 0, "error": "未启用", "updated_at": 0}) } // 启用的为连接中，否则禁用
}
async fn mcp_status(Query(query): Query<HashMap<String, String>>) -> ApiResult { // 查询连接状态，支持单服务与刷新
    let name = query.get("name").cloned().unwrap_or_default().trim().to_string(); // 指定服务名
    let items: Vec<Value> = mcp_store::load_servers().iter().map(mcp_store::normalize).collect(); // 归一化服务配置
    if !name.is_empty() { // 单服务查询
        let Some(target) = items.iter().find(|item| status_name(item) == name) else { return fail(404, "MCP 服务不存在"); }; // 找不到报错
        let status = mcp_store::status_of(&name).unwrap_or_else(|| status_default(target)); // 取状态，缺省默认值
        let mut map = Map::new(); // 状态映射
        map.insert(name, status); // 写入单项
        return ok(json!({"total": 1, "list": map})); // 返回单项结果
    }
    let refresh = query.get("refresh").map(|text| text == "1" || text.eq_ignore_ascii_case("true")).unwrap_or(false); // 是否要求重连
    if refresh { mcp_store::reconnect_all().await; } // 重连全部
    let mut list = mcp_store::statuses().as_object().cloned().unwrap_or_default(); // 取常驻状态
    list.retain(|key, _| items.iter().any(|item| status_name(item) == *key)); // 过滤已删除服务
    for item in &items { // 补齐缺失状态
        let item_name = status_name(item); // 服务名
        if item_name.is_empty() || list.contains_key(&item_name) { continue; } // 空名或已存在跳过
        list.insert(item_name, status_default(item)); // 写入默认状态
    }
    let total = list.len(); // 状态总数
    ok(json!({"total": total, "list": list})) // 返回全部状态
}
async fn create_mcp(body: Bytes) -> ApiResult { // 新建 MCP 服务
    let payload = body_object(&body); // 解析请求体
    let (data, error) = validate(&payload, None); // 校验配置
    if !error.is_empty() { return fail(400, &error); } // 校验失败
    let data = data.unwrap_or_else(|| json!({})); // 取归一化结果
    let mut items = mcp_store::load_servers(); // 读取服务列表
    if unique_check(&data, &items, "") { return fail(400, "该名称已存在"); } // 重名校验
    items.push(data); // 追加服务
    mcp_store::save_servers(&items); // 落盘
    mcp_store::spawn_reconnect_all(); // 后台重连全部
    list_response(&items) // 返回列表
}
async fn update_mcp(Path(name): Path<String>, body: Bytes) -> ApiResult { // 更新 MCP 服务
    let mut items = mcp_store::load_servers(); // 读取服务列表
    let Some(index) = find_index(&name) else { return fail(404, "MCP 服务不存在"); }; // 定位下标
    let payload = body_object(&body); // 解析请求体
    let existing = mcp_store::normalize(&items[index]); // 取原配置
    let (data, error) = validate(&payload, Some(&existing)); // 合并校验
    if !error.is_empty() { return fail(400, &error); } // 校验失败
    let data = data.unwrap_or_else(|| json!({})); // 取归一化结果
    if unique_check(&data, &items, &name) { return fail(400, "该名称已存在"); } // 重名校验
    items[index] = data; // 覆盖服务
    mcp_store::save_servers(&items); // 落盘
    mcp_store::spawn_reconnect_all(); // 后台重连全部
    list_response(&items) // 返回列表
}
async fn delete_mcp(Path(name): Path<String>) -> ApiResult { // 删除 MCP 服务
    let mut items = mcp_store::load_servers(); // 读取服务列表
    let Some(index) = find_index(&name) else { return fail(404, "MCP 服务不存在"); }; // 定位下标
    items.remove(index); // 删除服务
    mcp_store::save_servers(&items); // 落盘
    mcp_store::spawn_reconnect_all(); // 后台重连全部
    list_response(&items) // 返回列表
}
async fn toggle_mcp(Path(name): Path<String>, body: Bytes) -> ApiResult { // 切换服务启用状态
    let mut items = mcp_store::load_servers(); // 读取服务列表
    let Some(index) = find_index(&name) else { return fail(404, "MCP 服务不存在"); }; // 定位下标
    let mut item = mcp_store::normalize(&items[index]); // 取归一化配置
    let payload = body_object(&body); // 解析请求体
    item["enabled"] = json!(crate::state::as_bool(payload.get("enabled"), true)); // 写入启用状态
    items[index] = item; // 覆盖服务
    mcp_store::save_servers(&items); // 落盘
    mcp_store::spawn_reconnect(&name); // 后台重连该服务
    list_response(&items) // 返回列表
}
async fn get_mcp_json() -> ApiResult { // 导出脱敏配置 JSON
    let payload = json!({"servers": public_list()}); // 构造配置对象
    let text = format!("{}\n", serde_json::to_string_pretty(&payload).unwrap_or_default()); // 美化输出
    ok(json!({"text": text})) // 返回文本
}
async fn put_mcp_json(body: Bytes) -> ApiResult { // 整文件导入配置，兼容两种格式
    let payload = body_object(&body); // 解析请求体
    let text = obj_str(payload.as_object().unwrap(), "text"); // 取配置文本
    let parsed = match serde_json::from_str::<Value>(&text) { Ok(value) => value, Err(err) => return fail(400, &format!("JSON 格式错误：{err}")) }; // 解析 JSON
    let raw_items: Option<Vec<Value>> = if let Some(items) = parsed.get("servers").and_then(Value::as_array) { // servers 数组格式
        Some(items.clone())
    } else if let Some(map) = parsed.get("mcpServers").and_then(Value::as_object) { // mcpServers 映射格式
        Some(map.iter().map(|(name, item)| { // 逐个转为条目
            let mut entry = item.as_object().cloned().unwrap_or_default(); // 配置项
            entry.insert("name".to_string(), Value::String(name.clone())); // 注入名称
            Value::Object(entry) // 返回条目
        }).collect())
    } else { // 其它格式无效
        None
    };
    let Some(raw_items) = raw_items else { return fail(400, "顶层必须是 { \"servers\": [...] } 或 { \"mcpServers\": {...} }"); }; // 格式校验
    let existing: Map<String, Value> = mcp_store::load_servers().iter().map(mcp_store::normalize).filter_map(|item| item.get("name").and_then(Value::as_str).map(|name| (name.to_string(), item.clone()))).collect(); // 现有配置按名索引
    let mut items: Vec<Value> = Vec::new(); // 归一化结果
    let mut seen: Vec<String> = Vec::new(); // 名称去重集合
    for (index, raw) in raw_items.iter().enumerate() { // 逐项处理
        if !raw.is_object() { return fail(400, &format!("第 {} 项必须是对象", index + 1)); } // 非对象报错
        let name = mcp_store::normalize(raw).get("name").and_then(Value::as_str).unwrap_or_default().to_string(); // 取名称
        let (data, error) = validate(raw, existing.get(&name)); // 校验并保留原敏感值
        if !error.is_empty() { return fail(400, &format!("第 {} 项：{error}", index + 1)); } // 校验失败
        let data = data.unwrap_or_else(|| json!({})); // 归一化结果
        let data_name = data.get("name").and_then(Value::as_str).unwrap_or_default().to_string(); // 归一化后名称
        if seen.contains(&data_name) { return fail(400, &format!("第 {} 项：名称重复", index + 1)); } // 重名报错
        seen.push(data_name); // 记录名称
        items.push(data); // 写入结果
    }
    mcp_store::save_servers(&items); // 落盘
    mcp_store::spawn_reconnect_all(); // 后台重连全部
    crate::utils::logger::info("mcp_config_router", &format!("整文件更新 MCP 配置: {} 项", items.len())); // 记录更新日志
    list_response(&items) // 返回列表
}
