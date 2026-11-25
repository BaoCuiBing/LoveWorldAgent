use serde_json::{json, Value}; // JSON 结果
use std::path::Path; // adb 路径
use std::time::Duration; // 超时
use crate::mobile::adb::{self, ToolResult}; // adb 与错误
pub fn device_logs(adb: &Path, serial: &str, lines: usize) -> ToolResult<Value> { // 读取最近日志
    let count = lines.clamp(1, 2000).to_string(); // 行数限制
    let text = adb::shell(adb, serial, &["logcat", "-d", "-t", &count], Duration::from_secs(30))?; // logcat
    let items: Vec<&str> = text.lines().collect(); // 行
    Ok(json!({"lines": items, "total": items.len()})) // 返回
}
fn crash_blocks(text: &str) -> Vec<Value> { // 按 FATAL EXCEPTION 切块
    let mut blocks: Vec<Vec<String>> = Vec::new(); // 结果块
    let mut current: Vec<String> = Vec::new(); // 当前块
    for line in text.lines() { // 逐行
        if line.contains("FATAL EXCEPTION") { // 新崩溃
            if !current.is_empty() { blocks.push(std::mem::take(&mut current)); } // 收集上一块
        }
        if !current.is_empty() || line.contains("FATAL EXCEPTION") { current.push(line.to_string()); } // 追加
    }
    if !current.is_empty() { blocks.push(current); } // 收尾
    blocks.into_iter().enumerate().map(|(index, lines)| { // 转 JSON
        let head = lines.first().cloned().unwrap_or_default(); // 首行
        let package = head.split_whitespace().find(|item| item.contains('.')).unwrap_or("").to_string(); // 包名近似
        json!({"index": index, "package": package, "summary": head.trim(), "lines": lines, "count": lines.len()}) // 摘要
    }).collect() // 汇总
}
pub fn list_crashes(adb: &Path, serial: &str) -> ToolResult<Value> { // 崩溃列表
    let text = adb::shell(adb, serial, &["logcat", "-d", "-b", "crash", "-t", "500"], Duration::from_secs(30))?; // crash 缓冲区
    let blocks = crash_blocks(&text); // 解析
    Ok(json!({"total": blocks.len(), "crashes": blocks})) // 返回
}
pub fn get_crash(adb: &Path, serial: &str, index: Option<usize>) -> ToolResult<Value> { // 读取某次崩溃全文
    let text = adb::shell(adb, serial, &["logcat", "-d", "-b", "crash"], Duration::from_secs(40))?; // 全量 crash
    let blocks = crash_blocks(&text); // 解析
    let target = index.unwrap_or(blocks.len().saturating_sub(1)); // 默认最近一次
    let crash = blocks.get(target).cloned().ok_or_else(|| crate::mobile::adb::ToolError::new("INVALID_ARGUMENT", "没有可用的崩溃记录"))?; // 取块
    Ok(json!({"total": blocks.len(), "index": target, "crash": crash})) // 返回
}