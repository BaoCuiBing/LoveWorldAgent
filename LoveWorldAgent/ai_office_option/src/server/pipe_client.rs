use anyhow::{bail, Result}; // 错误类型
use serde_json::{json, Map, Value}; // JSON 构造
use std::fs::{File, OpenOptions}; // 管道句柄
use std::io::{BufRead, BufReader, Write}; // 行读写
use std::os::windows::fs::OpenOptionsExt; // 显式访问/共享模式
use std::path::PathBuf; // 路径类型
use std::sync::mpsc; // 截图超时通道
use std::time::{Duration, Instant}; // 重连间隔与耗时
use crate::com::constants::{EXCEL_PROGID, POWERPOINT_PROGID, WORD_PROGID}; // Office ProgID
use crate::com::dispatch::{attach_app, get as com_get}; // COM 附着与属性读（绝不创建新实例）
use crate::common::{paths::Paths, registry, screen}; // 路径、文档登记与窗口截图
use crate::server::agent_api; // 工具白名单校验
use crate::word::session; // Word STA 会话
use crate::{config, log_debug, log_info, log_warn}; // 配置与日志宏
const MAX_FRAME_BYTES: usize = 7400; // 单帧 data 转义后字节上限（留足顶层 screenshot 字段空间）
pub fn run(pipe_name: &str) -> Result<()> { // 以管道客户端模式运行
    let path = format!("\\\\.\\pipe\\{pipe_name}"); // 完整管道路径
    let mut attempts = 0u32; // 连续连接失败次数
    loop { // 断线自动重连
        match OpenOptions::new().read(true).write(true).access_mode(0x80000000 | 0x40000000).share_mode(0x1 | 0x2).open(&path) { // 连接主应用管道
            Ok(handle) => { // 连接成功
                attempts = 0; // 重置失败计数
                log_info!("pipe", "已连接主应用管道：{path}"); // 记录连接
                let mut writer = handle.try_clone()?; // 写句柄
                send_frame(&mut writer, &json!({"type": "ev", "event": "ready", "data": {"name": pipe_name}}))?; // 上线事件
                let reader = BufReader::new(handle); // 行读取器
                log_info!("pipe", "开始等待请求行"); // 读循环就绪
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
        std::thread::sleep(Duration::from_millis(500)); // 重试间隔
    }
}
fn family_of(cmd: &str) -> &'static str { // 依据工具前缀归族
    if cmd.starts_with("ppt_") { "ppt" } else if cmd.starts_with("excel_") { "excel" } else { "word" }
}
fn is_screenshot_action(cmd: &str, args: &Map<String, Value>) -> bool { // 过滤只读/关闭/退出/状态查询工具，仅在实质操作后截图
    if matches!(cmd, "shutdown" | "word_read" | "word_where_am_i" | "ppt_read" | "ppt_where_am_i" | "excel_read" | "excel_where_am_i") {
        return false;
    }
    let action = args.get("action").and_then(Value::as_str).unwrap_or_default();
    match cmd {
        "word_document" | "ppt_presentation" | "excel_workbook" => matches!(action, "create" | "open" | "save" | "save_as" | "export_pdf"),
        "word_find" => action != "find",
        "excel_formula" => action != "get",
        _ => cmd.starts_with("word_") || cmd.starts_with("ppt_") || cmd.starts_with("excel_"),
    }
}
fn resolve_active_hwnd(family: &str, args: &Map<String, Value>) -> isize { // 必须在所属 STA 线程内执行：仅附着已运行实例，绝不新建 Office
    let progid = match family {
        "ppt" => POWERPOINT_PROGID,
        "excel" => EXCEL_PROGID,
        _ => WORD_PROGID,
    };
    let raw_path = args
        .get("targetPath")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .or_else(|| args.get("path").and_then(Value::as_str))
        .unwrap_or_default();
    let entry = if !raw_path.trim().is_empty() {
        Paths::load()
            .resolve(raw_path)
            .ok()
            .and_then(|full| registry::get(&full.to_string_lossy(), progid))
    } else {
        None
    };
    let (app, doc) = match entry {
        Some(hit) => (hit.app, Some(hit.doc)),
        None => match attach_app(progid) {
            Ok(app) => (app, None),
            Err(_) => return 0,
        },
    };
    let visible = com_get(&app, "Visible").map(|v| v.as_bool()).unwrap_or(true);
    if !visible {
        return 0;
    }
    let collection_name = match family {
        "ppt" => "Presentations",
        "excel" => "Workbooks",
        _ => "Documents",
    };
    let open_count = com_get(&app, collection_name)
        .and_then(|c| com_get(&c.as_dispatch()?, "Count"))
        .map(|v| v.as_i32())
        .unwrap_or(0);
    if open_count <= 0 {
        return 0;
    }
    let read_hwnd = |obj: &crate::com::dispatch::ComObj| -> isize {
        com_get(obj, "Hwnd").map(|v| v.as_isize()).unwrap_or(0)
    };
    let active_win_hwnd = |obj: &crate::com::dispatch::ComObj| -> isize {
        com_get(obj, "ActiveWindow")
            .ok()
            .and_then(|v| v.as_dispatch().ok())
            .map(|w| read_hwnd(&w))
            .unwrap_or(0)
    };
    let raw_hwnd = match family {
        "ppt" => {
            let h = active_win_hwnd(&app);
            if h > 0 { h } else { read_hwnd(&app) }
        }
        "excel" => {
            let h = active_win_hwnd(&app);
            if h > 0 { screen::root_window(h) } else { screen::root_window(read_hwnd(&app)) }
        }
        _ => {
            let h = doc.as_ref().map(active_win_hwnd).unwrap_or(0);
            if h > 0 { h } else { active_win_hwnd(&app) }
        }
    };
    if raw_hwnd > 0 { screen::root_window(raw_hwnd) } else { 0 }
}
fn resolve_hwnd_on_sta(family: &str, args: &Map<String, Value>) -> isize { // 在所属 STA 线程解析当前窗口句柄（2s 超时）
    let fam = family.to_string();
    let args_clone = args.clone();
    let handle = match family {
        "ppt" => crate::ppt::handle(),
        "excel" => crate::excel::handle(),
        _ => session::handle(),
    };
    handle
        .submit_fn(Duration::from_secs(2), move || resolve_active_hwnd(&fam, &args_clone))
        .unwrap_or(0)
}
fn capture_error_isolated<F>(capture_fn: F) -> Option<String>
where
    F: FnOnce() -> Result<PathBuf>,
{ // 隔离截图异常，绝不影响主 Office 操作返回值
    match capture_fn() {
        Ok(path) => Some(path.to_string_lossy().to_string()),
        Err(err) => {
            log_warn!("pipe", "Office 自动截图跳过：{err}");
            None
        }
    }
}
fn capture_with_timeout(family: &'static str, hwnd: isize, timeout: Duration) -> Option<String> { // 脱离 STA 线程执行 GDI 抓图并限 3s 超时
    let (tx, rx) = mpsc::sync_channel::<Option<String>>(1);
    let Ok(_) = std::thread::Builder::new().name("office-shot".into()).spawn(move || {
        let res = capture_error_isolated(|| {
            let png = screen::capture_window_png(hwnd)?;
            let filename = screen::next_screenshot_filename(family, screen::next_screenshot_seq());
            screen::save_png_to_dir(&config::screenshot_dir(), &filename, &png)
        });
        let _ = tx.send(res);
    }) else {
        return None;
    };
    match rx.recv_timeout(timeout) {
        Ok(path) => path,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            log_warn!("pipe", "Office 自动截图超时（3s），已跳过");
            None
        }
        Err(_) => None,
    }
}
fn handle_line(writer: &mut File, line: &str) -> Result<()> { // 处理一行请求帧
    let frame: Value = match serde_json::from_str(line) { Ok(value) => value, Err(err) => { bail!("无效 JSON: {err}"); } }; // 解析失败直接回错误
    let kind = frame.get("type").and_then(|value| value.as_str()).unwrap_or("req"); // 帧类型
    if kind != "req" { log_debug!("pipe", "忽略非请求帧：{kind}"); return Ok(()); } // 仅处理 req
    let id = frame.get("id").and_then(|value| value.as_i64()).unwrap_or(0); // 请求 id
    let cmd = frame.get("cmd").and_then(|value| value.as_str()).unwrap_or_default().to_string(); // 工具名
    if cmd.is_empty() { bail!("缺少 cmd"); } // 缺少工具名
    let family = family_of(&cmd); // 工具所属族
    let auto_shot = frame.get("auto_screenshot").and_then(Value::as_bool).unwrap_or_else(|| config::auto_screenshot_enabled(family)); // 优先请求帧开关，否则读本地配置
    let args: Map<String, Value> = frame.get("args").and_then(|value| value.as_object()).cloned().unwrap_or_default(); // 参数
    if !agent_api::is_known(&cmd) { bail!("未知工具：{cmd}"); } // 白名单校验
    let action = args.get("action").or_else(|| args.get("target")).and_then(|value| value.as_str()).unwrap_or("").to_string(); // 动作/目标
    let path = args.get("path").and_then(|value| value.as_str()).unwrap_or("").to_string(); // 文档路径
    if cmd == "shutdown" { // 关闭命令
        log_info!("pipe", "收到 shutdown，结束受控 Office 实例并回复 ack（id={id}）"); // 记录
        let mut quit_args = Map::new(); // 退出参数
        quit_args.insert("action".to_string(), json!("quit")); // quit
        let _ = session::submit("word_document", &quit_args, Duration::from_secs(5)); // Word 受控实例
        let _ = crate::ppt::run("ppt_presentation", &quit_args); // PPT 受控实例
        let _ = crate::excel::run("excel_workbook", &quit_args); // Excel 受控实例
        send_envelope(writer, id, &json!({"ok": true, "data": {"shutdown": true}}), None)?; // ack
        std::process::exit(0); // 退出（COM 由 STA 线程退出时 CoUninitialize）
    }
    log_info!("pipe", "收到请求 id={id} cmd={cmd} action={action} path={path}"); // 请求记录（含动作与路径）
    let started = Instant::now(); // 计时
    let timeout = agent_api::timeout_of(&cmd); // 按工具选超时
    let value = if cmd.starts_with("ppt_") { // PPT 族独立 STA
        crate::ppt::run(&cmd, &args)
    } else if cmd.starts_with("excel_") { // Excel 族独立 STA
        crate::excel::run(&cmd, &args)
    } else { // Word 族
        session::submit(&cmd, &args, timeout)
    };
    let duration_ms = started.elapsed().as_millis(); // 耗时
    let ok = value.get("ok").and_then(|item| item.as_bool()).unwrap_or(false); // 成功判定
    let error_code = value.get("error").and_then(|item| item.get("code")).and_then(|item| item.as_str()).unwrap_or("").to_string(); // 错误码
    log_info!("pipe", "工具 {cmd} action={action} path={path} 完成，耗时 {duration_ms}ms，ok={ok}，errorCode={error_code}（id={id}）"); // 结构化审计
    let screenshot = if ok && auto_shot && is_screenshot_action(&cmd, &args) {
        let hwnd = resolve_hwnd_on_sta(family, &args);
        if screen::is_capturable_window(hwnd) {
            std::thread::sleep(Duration::from_millis(100));
            capture_with_timeout(family, hwnd, Duration::from_secs(3))
        } else {
            None
        }
    } else {
        None
    };
    send_envelope(writer, id, &value, screenshot.as_deref()) // 直接回传工具信封（screenshot 仅放响应帧顶层，绝不入 data）
}
fn build_envelope_frames(id: i64, value: &Value, screenshot: Option<&str>) -> Result<Vec<Value>> { // 组装响应帧列表（screenshot 仅放单帧或末分片顶层，绝不写入 data）
    let shot = screenshot.map(str::trim).filter(|s| !s.is_empty());
    let mut frame = value.as_object().cloned().unwrap_or_default(); // 复制信封
    frame.insert("type".into(), json!("res")); // 帧类型
    frame.insert("id".into(), json!(id)); // 请求 id
    let envelope = Value::Object(frame); // 组装帧
    let text = serde_json::to_string(&envelope)?; // 序列化
    if text.len() > MAX_FRAME_BYTES { // 分片
        let parts = split_utf8(&text, MAX_FRAME_BYTES); // 切片
        let total = parts.len(); // 总数
        let ok = value.get("ok").and_then(|item| item.as_bool()).unwrap_or(false); // 成功标记
        let error = value.get("error").cloned().unwrap_or(Value::Null); // 错误对象
        let mut frames = Vec::with_capacity(total);
        for (index, part) in parts.iter().enumerate() { // 逐片
            let mut chunk = json!({"type": "res", "id": id, "ok": ok, "error": error, "chunks": total, "chunk": index, "data": part});
            if index + 1 == total {
                if let Some(path) = shot { chunk["screenshot"] = json!(path); }
            }
            frames.push(chunk);
        }
        return Ok(frames); // 分片完成
    }
    let mut single = envelope;
    if let Some(path) = shot {
        if let Some(map) = single.as_object_mut() {
            map.insert("screenshot".into(), json!(path));
        }
    }
    Ok(vec![single]) // 单帧
}
fn send_envelope(writer: &mut File, id: i64, value: &Value, screenshot: Option<&str>) -> Result<()> { // 将工具信封包装为响应帧（含 id/type/可选顶层 screenshot）
    for frame in build_envelope_frames(id, value, screenshot)? {
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
fn split_utf8(text: &str, limit: usize) -> Vec<String> { // 按 UTF-8 字符边界切分，并保证每片 JSON 转义后不超过 limit
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
    fn args_with_action(action: &str) -> Map<String, Value> {
        let mut map = Map::new();
        if !action.is_empty() {
            map.insert("action".into(), json!(action));
        }
        map
    }
    #[test]
    fn screenshot_action_filter_excludes_readonly_and_close_quit() {
        let empty = Map::new();
        for readonly in ["shutdown", "word_read", "word_where_am_i", "ppt_read", "ppt_where_am_i", "excel_read", "excel_where_am_i"] {
            assert!(!is_screenshot_action(readonly, &empty), "{readonly} must not capture");
        }
        for lifecycle_skip in ["status", "info", "structure", "close", "quit"] {
            let args = args_with_action(lifecycle_skip);
            assert!(!is_screenshot_action("word_document", &args));
            assert!(!is_screenshot_action("ppt_presentation", &args));
            assert!(!is_screenshot_action("excel_workbook", &args));
        }
        for lifecycle_capture in ["create", "open", "save", "save_as", "export_pdf"] {
            let args = args_with_action(lifecycle_capture);
            assert!(is_screenshot_action("word_document", &args));
            assert!(is_screenshot_action("ppt_presentation", &args));
            assert!(is_screenshot_action("excel_workbook", &args));
        }
        assert!(!is_screenshot_action("word_find", &args_with_action("find")));
        assert!(is_screenshot_action("word_find", &args_with_action("replace")));
        assert!(!is_screenshot_action("excel_formula", &args_with_action("get")));
        assert!(is_screenshot_action("excel_formula", &args_with_action("set")));
        for mutating in ["word_write", "word_table", "word_format", "word_structure", "word_edit", "word_media", "word_variable", "word_batch", "ppt_slide", "ppt_write", "ppt_shape", "ppt_table", "ppt_format", "ppt_chart", "ppt_media", "excel_write", "excel_sheet", "excel_format", "excel_table", "excel_chart", "excel_sort_filter"] {
            assert!(is_screenshot_action(mutating, &empty), "{mutating} should capture");
        }
    }
    #[test]
    fn envelope_frames_put_screenshot_at_top_level_never_inside_data() {
        let single_val = json!({"ok": true, "data": {"path": "C:\\doc.docx"}});
        let single = build_envelope_frames(1, &single_val, Some("C:\\static\\data\\office_screen\\word_1.png")).unwrap();
        assert_eq!(single.len(), 1);
        assert_eq!(single[0]["screenshot"], json!("C:\\static\\data\\office_screen\\word_1.png"));
        assert!(single[0]["data"].get("screenshot").is_none());
        let big_val = json!({"ok": true, "data": {"text": "a".repeat(MAX_FRAME_BYTES + 800)}});
        let chunked = build_envelope_frames(2, &big_val, Some("C:\\static\\data\\office_screen\\word_2.png")).unwrap();
        assert!(chunked.len() > 1);
        for (idx, frame) in chunked.iter().enumerate() {
            let serialized = serde_json::to_string(frame).unwrap();
            assert!(serialized.len() < 8192, "chunk {idx} too large: {}", serialized.len());
            if idx + 1 < chunked.len() {
                assert!(frame.get("screenshot").is_none());
            } else {
                assert_eq!(frame["screenshot"], json!("C:\\static\\data\\office_screen\\word_2.png"));
            }
        }
        let joined: String = chunked.iter().filter_map(|f| f.get("data").and_then(Value::as_str)).collect();
        let reassembled: Value = serde_json::from_str(&joined).unwrap();
        assert!(reassembled.get("screenshot").is_none());
        assert!(reassembled["data"].get("screenshot").is_none());
    }
    #[test]
    fn capture_error_isolation_returns_none_on_failure() {
        let res = capture_error_isolated(|| bail!("simulated capture error"));
        assert!(res.is_none());
    }
}
