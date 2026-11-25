use anyhow::{bail, Result}; // 错误类型
use serde_json::{json, Map, Value}; // JSON 构造
use std::fs::{File, OpenOptions}; // 管道句柄
use std::io::{BufRead, BufReader, Write}; // 行读写
use std::path::PathBuf; // 路径类型
use std::time::{Duration, Instant}; // 重连间隔与耗时
use windows::Win32::UI::WindowsAndMessaging::{IsIconic, IsWindow, IsWindowVisible}; // 窗口可见性校验
use crate::{config, log_debug, log_info, log_warn}; // 配置与日志宏
use crate::server::{agent_api, apps, snapshots}; // 工具实现与窗口解析
use crate::sys::{overlay, screen}; // 覆盖层与屏幕采集
use crate::uia::{desktop, worker::UiaHandle}; // 桌面句柄与 UIA 句柄
const MAX_FRAME_BYTES: usize = 7400; // 单帧 data 转义后字节上限（留足顶层 screenshot 字段空间）
pub fn run(uia: UiaHandle, pipe_name: &str) -> Result<()> { // 以管道客户端模式运行
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?; // 单线程运行时
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
                        Ok(text) => { let text = text.trim().to_string(); if text.is_empty() { continue; } if let Err(err) = handle_line(&runtime, &uia, &mut writer, &text) { log_warn!("pipe", "处理请求失败：{err}"); } } // 正常处理
                        Err(err) => { log_warn!("pipe", "读取管道失败：{err}"); break; } // 读失败退出内层
                    }
                }
                overlay::hide(); // 断线立即隐藏覆盖层
                log_warn!("pipe", "管道已断开，准备重连"); // 断线提示
            }
            Err(err) => { attempts += 1; if attempts == 1 || attempts % 60 == 0 { log_warn!("pipe", "连接管道失败（第 {attempts} 次）：{err}，500ms 后重试"); } else { log_debug!("pipe", "连接管道失败（第 {attempts} 次）：{err}"); } } // 连接失败（限流记录）
        }
        std::thread::sleep(Duration::from_millis(500)); // 重试间隔
    }
}
fn is_screenshot_action(cmd: &str) -> bool { // 仅对 14 个桌面/窗口变更操作触发自动截图（排除 9 个只读/仅移动光标工具）
    matches!(
        cmd,
        "computer_click"
            | "computer_double_click"
            | "computer_right_click"
            | "computer_drag"
            | "computer_type_text"
            | "computer_press_key"
            | "computer_hotkey"
            | "computer_scroll"
            | "computer_set_value"
            | "computer_invoke_menu"
            | "computer_launch_app"
            | "computer_kill_app"
            | "computer_bring_to_front"
            | "computer_set_window_frame"
    )
}
fn is_capturable_window(hwnd: i64) -> bool { // 校验窗口有效、可见且未最小化
    if hwnd <= 0 { return false; }
    let target = desktop::hwnd_of(hwnd);
    unsafe { IsWindow(Some(target)).as_bool() && IsWindowVisible(target).as_bool() && !IsIconic(target).as_bool() }
}
fn resolve_capture_hwnd(cmd: &str, args: &Map<String, Value>, data: Option<&Value>) -> Option<i64> { // 解析目标窗口 HWND；桌面操作或无可见窗返回 None
    if cmd == "computer_kill_app" { return None; }
    let is_desktop_scope = args.get("scope").and_then(Value::as_str).map(|s| s.eq_ignore_ascii_case("desktop")).unwrap_or(false);
    if is_desktop_scope && args.get("pid").is_none() && args.get("window_id").is_none() { return None; }
    let pid_arg = args.get("pid").and_then(Value::as_u64).map(|p| p as u32);
    let candidate = data
        .and_then(|d| d.get("window_id"))
        .and_then(Value::as_i64)
        .or_else(|| data.and_then(|d| d.get("windows")).and_then(Value::as_array).and_then(|arr| arr.first()).and_then(|w| w.get("window_id")).and_then(Value::as_i64))
        .or_else(|| args.get("window_id").and_then(Value::as_i64))
        .or_else(|| args.get("element_token").and_then(Value::as_str).and_then(|token| snapshots::resolve_token(token, pid_arg).ok().map(|resolved| resolved.hwnd)))
        .or_else(|| pid_arg.and_then(|pid| apps::resolve_pid_hwnd(Some(pid), None).ok().map(|(_, hwnd)| hwnd)));
    candidate.filter(|&hwnd| is_capturable_window(hwnd))
}
fn capture_and_save_step(target_hwnd: Option<i64>) -> Result<PathBuf> { // 截取目标窗口（失败或无窗口回退虚拟桌面）并写入 computer_screen 目录
    let (png, _, _, _, _) = match target_hwnd {
        Some(hwnd) => screen::capture_window_png(hwnd).or_else(|_| screen::capture_desktop_png())?,
        None => screen::capture_desktop_png()?,
    };
    let filename = screen::next_screenshot_filename("cu", screen::next_screenshot_seq());
    screen::save_png_to_dir(&config::screenshot_dir(), &filename, &png)
}
fn capture_error_isolated<F>(capture_fn: F) -> Option<String>
where
    F: FnOnce() -> Result<PathBuf>,
{ // 隔离截图异常，绝不影响主桌面操作返回值
    match capture_fn() {
        Ok(path) => Some(path.to_string_lossy().to_string()),
        Err(err) => {
            log_warn!("pipe", "自动截图跳过：{err}");
            None
        }
    }
}
fn handle_line(runtime: &tokio::runtime::Runtime, uia: &UiaHandle, writer: &mut File, line: &str) -> Result<()> { // 处理一行请求帧
    let started = Instant::now(); // 记录耗时
    let frame: Value = match serde_json::from_str(line) { Ok(value) => value, Err(err) => { send_res(writer, 0, false, None, Some(&format!("无效 JSON: {err}")), None)?; bail!("无效 JSON: {err}"); } }; // 解析失败直接回错误
    let kind = frame.get("type").and_then(|value| value.as_str()).unwrap_or("req"); // 帧类型
    if kind != "req" { log_debug!("pipe", "忽略非请求帧：{kind}"); return Ok(()); } // 仅处理 req
    let id = frame.get("id").and_then(|value| value.as_i64()).unwrap_or(0); // 请求 id
    let cmd = frame.get("cmd").and_then(|value| value.as_str()).unwrap_or_default().to_string(); // 工具名
    if cmd.is_empty() { send_res(writer, id, false, None, Some("缺少 cmd"), None)?; bail!("缺少 cmd"); } // 缺少工具名
    let auto_shot = frame.get("auto_screenshot").and_then(Value::as_bool).unwrap_or_else(config::auto_screenshot_enabled); // 优先读取请求帧下发的开关，否则读本地配置
    let args: Map<String, Value> = frame.get("args").and_then(|value| value.as_object()).cloned().unwrap_or_default(); // 参数
    log_info!("pipe", "收到请求 id={id} cmd={cmd}"); // 请求记录（INFO 便于生产审计）
    let args_for_shot = args.clone(); // 供操作后解析窗口句柄
    let value = runtime.block_on(async { match tokio::time::timeout(Duration::from_secs(90), agent_api::call(uia, &cmd, args)).await { Ok(value) => value, Err(_) => json!({"ok": false, "error": "TIMEOUT：工具执行超过 90 秒"}) } }); // 同步等待工具结果（留足 omni_parser 首次加载与 UIA 遍历耗时）
    let ok = value.get("ok").and_then(|item| item.as_bool()).unwrap_or(false); // 成功判定
    if ok { log_info!("pipe", "工具 {cmd} 完成，耗时 {}ms（id={id}）", started.elapsed().as_millis()); } else { log_warn!("pipe", "工具 {cmd} 失败，耗时 {}ms（id={id}）：{}", started.elapsed().as_millis(), value.get("error").and_then(|item| item.as_str()).unwrap_or("未知错误")); } // 耗时记录
    let screenshot = if ok && auto_shot && is_screenshot_action(&cmd) {
        let target_hwnd = resolve_capture_hwnd(&cmd, &args_for_shot, value.get("data"));
        runtime.block_on(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            match tokio::time::timeout(Duration::from_secs(3), tokio::task::spawn_blocking(move || capture_error_isolated(|| capture_and_save_step(target_hwnd)))).await {
                Ok(Ok(path)) => path,
                Ok(Err(err)) => { log_warn!("pipe", "自动截图任务异常：{err}"); None }
                Err(_) => { log_warn!("pipe", "自动截图超时（3s），已跳过"); None }
            }
        })
    } else {
        None
    };
    if ok { send_res(writer, id, true, Some(value.get("data").unwrap_or(&Value::Null)), None, screenshot.as_deref()) } else { send_res(writer, id, false, None, Some(value.get("error").and_then(|item| item.as_str()).unwrap_or("未知错误")), None) } // 成功帧 data 只放工具负载，screenshot 仅放顶层字段
}
fn build_res_frames(id: i64, ok: bool, data: Option<&Value>, error: Option<&str>, screenshot: Option<&str>) -> Result<Vec<Value>> { // 组装响应帧列表（screenshot 仅放顶层，绝不写入 data）
    let shot = screenshot.map(str::trim).filter(|s| !s.is_empty());
    if let Some(value) = data { // 成功分支
        let text = serde_json::to_string(value)?; // 序列化数据
        if text.len() > MAX_FRAME_BYTES { // 需要分片
            let parts = split_utf8(&text, MAX_FRAME_BYTES); // 按 UTF-8 边界切片
            let total = parts.len(); // 分片总数
            let mut frames = Vec::with_capacity(total);
            for (index, part) in parts.iter().enumerate() { // 逐片下发
                let mut frame = json!({"type": "res", "id": id, "ok": ok, "chunks": total, "chunk": index, "data": part});
                if index + 1 == total {
                    if let Some(path) = shot { frame["screenshot"] = json!(path); }
                }
                frames.push(frame);
            }
            return Ok(frames); // 分片完成
        }
        let mut frame = json!({"type": "res", "id": id, "ok": ok, "data": value});
        if let Some(path) = shot { frame["screenshot"] = json!(path); }
        return Ok(vec![frame]); // 单帧回包
    }
    let message = error.unwrap_or("未知错误"); // 错误文本
    Ok(vec![json!({"type": "res", "id": id, "ok": ok, "error": message})]) // 错误帧
}
fn send_res(writer: &mut File, id: i64, ok: bool, data: Option<&Value>, error: Option<&str>, screenshot: Option<&str>) -> Result<()> { // 组装并发送响应
    for frame in build_res_frames(id, ok, data, error, screenshot)? {
        send_frame(writer, &frame)?;
    }
    Ok(())
}
fn send_frame(writer: &mut File, frame: &Value) -> Result<()> { // 写一帧并刷送
    let text = serde_json::to_string(frame)?; // 序列化帧
    writer.write_all(text.as_bytes())?; // 写正文
    writer.write_all(b"\n")?; // 帧分隔
    writer.flush()?; // 立即刷送
    Ok(())
}
fn split_utf8(text: &str, limit: usize) -> Vec<String> { // 按 UTF-8 字符边界切分，并保证每片 JSON 转义后不超过 limit（否则分片帧整体 >8192 会被主应用丢弃导致永久阻塞）
    let mut parts: Vec<String> = Vec::new(); // 分片容器
    let mut current = String::new(); // 当前分片
    let mut current_bytes = 0usize; // 当前分片转义后字节数
    for ch in text.chars() { // 逐字符累计
        let escaped = match ch { // serde_json 转义宽度
            '"' | '\\' => 2, // 引号与反斜杠
            '\n' | '\r' | '\t' | '\u{08}' | '\u{0c}' => 2, // 短转义序列
            c if (c as u32) < 0x20 => 6, // \u00XX 控制字符
            c => c.len_utf8(), // 其余按原字节
        };
        if current_bytes + escaped > limit && !current.is_empty() { // 超限先封箱
            parts.push(std::mem::take(&mut current)); // 收集分片
            current_bytes = 0; // 重新计数
        }
        current.push(ch); // 追加字符
        current_bytes += escaped; // 累计转义宽度
    }
    if !current.is_empty() { parts.push(current); } // 收尾分片
    parts
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn screenshot_action_filter_excludes_readonly_tools() {
        let mutating = [
            "computer_click",
            "computer_double_click",
            "computer_right_click",
            "computer_drag",
            "computer_type_text",
            "computer_press_key",
            "computer_hotkey",
            "computer_scroll",
            "computer_set_value",
            "computer_invoke_menu",
            "computer_launch_app",
            "computer_kill_app",
            "computer_bring_to_front",
            "computer_set_window_frame",
        ];
        let readonly = [
            "computer_list_apps",
            "computer_list_windows",
            "computer_get_window_state",
            "computer_get_desktop_state",
            "computer_debug_window_info",
            "computer_get_accessibility_tree",
            "computer_get_screen_size",
            "computer_get_cursor_position",
            "computer_move_cursor",
        ];
        for tool in mutating {
            assert!(is_screenshot_action(tool), "{tool} should trigger auto-screenshot");
        }
        for tool in readonly {
            assert!(!is_screenshot_action(tool), "{tool} must NOT trigger auto-screenshot");
        }
    }
    #[test]
    fn res_frame_puts_screenshot_at_top_level_never_inside_data() {
        let data = json!({"window_id": 42, "clicked": true});
        let single = build_res_frames(7, true, Some(&data), None, Some("C:\\tmp\\cu_1.png")).unwrap();
        assert_eq!(single.len(), 1);
        assert_eq!(single[0]["screenshot"], json!("C:\\tmp\\cu_1.png"));
        assert!(single[0]["data"].get("screenshot").is_none());
        let big = json!({"text": "a".repeat(MAX_FRAME_BYTES + 500)});
        let chunked = build_res_frames(8, true, Some(&big), None, Some("C:\\tmp\\cu_2.png")).unwrap();
        assert!(chunked.len() > 1);
        for (idx, frame) in chunked.iter().enumerate() {
            if idx + 1 < chunked.len() {
                assert!(frame.get("screenshot").is_none());
            } else {
                assert_eq!(frame["screenshot"], json!("C:\\tmp\\cu_2.png"));
            }
        }
    }
    #[test]
    fn capture_error_isolation_returns_none_on_failure() {
        let res = capture_error_isolated(|| bail!("simulated capture failure"));
        assert!(res.is_none());
    }
}
