use anyhow::{anyhow, bail, Result}; // 错误类型
use serde_json::{json, Map, Value}; // JSON 构造
use std::fs::{File, OpenOptions}; // 管道句柄
use std::io::{BufRead, BufReader, Write}; // 行读写
use std::time::Instant; // 耗时
use crate::{log_debug, log_info, log_warn}; // 日志宏
use crate::server::agent_api; // 工具实现
const MAX_FRAME_BYTES: usize = 8000; // 单帧 data 转义后字节上限（帧整体须 <8192）
pub fn run(pipe_name: &str) -> Result<()> { // 以管道客户端模式运行
    let path = format!("\\\\.\\pipe\\{pipe_name}"); // 完整管道路径
    let mut attempts = 0u32; // 连续连接失败次数
    loop { // 断线自动重连
        match OpenOptions::new().read(true).write(true).open(&path) { // 连接主应用管道
            Ok(handle) => { // 连接成功
                attempts = 0; // 重置失败计数
                log_info!("pipe", "已连接主应用管道：{path}"); // 记录连接
                let mut writer = handle.try_clone()?; // 写句柄
                send_frame(&mut writer, &json!({"type": "ev", "event": "ready", "data": {"name": pipe_name}}))?; // 上线事件
                let reader = BufReader::new(handle); // 行读取器
                for line in reader.lines() { // 逐行处理
                    match line { // 读取结果
                        Ok(text) => { let text = text.trim().to_string(); if text.is_empty() { continue; } if let Err(err) = handle_line(&mut writer, &text) { log_warn!("pipe", "处理请求失败：{err}"); } } // 正常处理
                        Err(err) => { log_warn!("pipe", "读取管道失败：{err}"); break; } // 读失败退出内层
                    }
                }
                log_warn!("pipe", "管道已断开，准备重连"); // 断线提示
            }
            Err(err) => { attempts += 1; if attempts == 1 || attempts % 60 == 0 { log_warn!("pipe", "连接管道失败（第 {attempts} 次）：{err}，500ms 后重试"); } else { log_debug!("pipe", "连接管道失败（第 {attempts} 次）：{err}"); } } // 连接失败（限流记录）
        }
        std::thread::sleep(std::time::Duration::from_millis(500)); // 重试间隔
    }
}
fn handle_line(writer: &mut File, line: &str) -> Result<()> { // 处理一行请求帧
    let frame: Value = match serde_json::from_str(line) { Ok(value) => value, Err(err) => { send_res(writer, 0, false, None, Some(&format!("无效 JSON: {err}")))?; bail!("无效 JSON: {err}"); } }; // 解析失败回错误
    let kind = frame.get("type").and_then(|value| value.as_str()).unwrap_or("req"); // 帧类型
    if kind != "req" { log_debug!("pipe", "忽略非请求帧：{kind}"); return Ok(()); } // 仅处理 req
    let id = frame.get("id").and_then(Value::as_i64).unwrap_or(0); // 请求 id
    let cmd = frame.get("cmd").and_then(Value::as_str).unwrap_or_default().to_string(); // 工具名
    if cmd.is_empty() { send_res(writer, id, false, None, Some("缺少 cmd"))?; bail!("缺少 cmd"); } // 缺少工具名
    if !agent_api::is_known(&cmd) { send_res(writer, id, false, None, Some(&format!("未知工具：{cmd}")))?; bail!("未知工具：{cmd}"); } // 白名单校验
    let args: Map<String, Value> = frame.get("args").and_then(Value::as_object).cloned().unwrap_or_default(); // 参数
    log_info!("pipe", "收到请求 id={id} cmd={cmd}"); // 请求记录
    let started = Instant::now(); // 计时
    let value = agent_api::call(&cmd, &args); // 执行工具
    let ok = value.get("ok").and_then(Value::as_bool).unwrap_or(false); // 成功判定
    if ok { log_info!("pipe", "工具 {cmd} 完成，耗时 {}ms（id={id}）", started.elapsed().as_millis()); } else { log_warn!("pipe", "工具 {cmd} 失败，耗时 {}ms（id={id}）：{}", started.elapsed().as_millis(), value.get("error").and_then(Value::as_str).unwrap_or("未知错误")); } // 耗时记录
    if ok { send_res(writer, id, true, value.get("data"), None) } else { send_res(writer, id, false, None, Some(value.get("error").and_then(Value::as_str).unwrap_or("未知错误"))) } // 回包
}
fn send_res(writer: &mut File, id: i64, ok: bool, data: Option<&Value>, error: Option<&str>) -> Result<()> { // 组装响应
    if let Some(value) = data { // 成功分支
        let text = serde_json::to_string(value)?; // 序列化
        if text.len() > MAX_FRAME_BYTES { // 需要分片
            let parts = split_utf8(&text, MAX_FRAME_BYTES); // 分片
            let total = parts.len(); // 总数
            for (index, part) in parts.iter().enumerate() { send_frame(writer, &json!({"type": "res", "id": id, "ok": ok, "chunks": total, "chunk": index, "data": part}))?; } // 逐片下发
            return Ok(()); // 完成
        }
        return send_frame(writer, &json!({"type": "res", "id": id, "ok": ok, "data": value})); // 单帧
    }
    let message = error.unwrap_or("未知错误"); // 错误文本
    send_frame(writer, &json!({"type": "res", "id": id, "ok": ok, "error": message})) // 错误帧
}
fn send_frame(writer: &mut File, frame: &Value) -> Result<()> { // 写一帧
    let text = serde_json::to_string(frame).map_err(|err| anyhow!("序列化帧失败：{err}"))?; // 序列化
    writer.write_all(text.as_bytes())?; // 正文
    writer.write_all(b"\n")?; // 分隔
    writer.flush()?; // 刷送
    Ok(()) // 完成
}
fn split_utf8(text: &str, limit: usize) -> Vec<String> { // 按 UTF-8 边界分片
    let mut parts: Vec<String> = Vec::new(); // 分片容器
    let mut current = String::new(); // 当前分片
    let mut current_bytes = 0usize; // 当前转义字节数
    for ch in text.chars() { // 逐字符
        let escaped = match ch { '"' | '\\' => 2, '\n' | '\r' | '\t' | '\u{08}' | '\u{0c}' => 2, c if (c as u32) < 0x20 => 6, c => c.len_utf8() }; // 转义宽度
        if current_bytes + escaped > limit && !current.is_empty() { parts.push(std::mem::take(&mut current)); current_bytes = 0; } // 超限封箱
        current.push(ch); // 追加
        current_bytes += escaped; // 累计
    }
    if !current.is_empty() { parts.push(current); } // 收尾
    parts // 返回
}