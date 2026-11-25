use serde_json::{json, Value}; // JSON 结果
use std::path::Path; // adb 路径
use crate::mobile::adb::{self, ToolResult}; // adb 与错误
pub fn list(adb: &Path) -> ToolResult<Value> { // 列出在线设备
    let devices = adb::devices(adb)?; // 查询
    let items: Vec<Value> = devices.into_iter().map(|item| json!({"serial": item.serial, "model": item.model, "state": "device"})).collect(); // 转 JSON
    Ok(json!({"devices": items, "total": items.len()})) // 返回
}