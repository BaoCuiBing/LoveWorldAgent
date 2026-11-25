use std::path::PathBuf; // 可见终端工具集：terminal_open/send/read/list，会话由本进程 PTY 托管并通过 hook 下发终端事件
use std::sync::atomic::Ordering; // 原子标志读取
use std::time::{Duration, Instant}; // 超时控制与计时
use serde_json::{json, Value}; // JSON 构造与动态值
use crate::agent::tools::{params, blocking, ToolDef}; // 工具定义与同步包装
use crate::agent::tools_details::common::{clean_terminal_output, notify_terminal, read_since, remember_tool_meta, resolve_in_root, session_alive, session_by_id, terminal_sessions, truncate_chars, write_session, TerminalSession, NO_PROJECT, OUTPUT_LIMIT, QUIET_SECONDS}; // 终端会话公共函数
fn arg(args: &str, key: &str) -> String { // 从参数 JSON 读取字符串字段
    serde_json::from_str::<Value>(args).ok().and_then(|value| value.get(key).and_then(Value::as_str).map(str::to_string)).unwrap_or_default() // 解析失败返回空串
}
fn root_text(root: &std::path::Path) -> String { // 规范化项目根为字符串标识（去除 Windows \\?\ 前缀）
    crate::agent::tools_details::common::normalized_dir_text(root) // 与 open_terminal 保持完全一致
}
pub fn build_terminal_tools(root: Option<PathBuf>, timeout_seconds: i64) -> Vec<ToolDef> { // 构造四个终端工具
    let base = root.clone(); // 克隆项目根供 open 使用
    let open = ToolDef::new("terminal_open", "在右栏可见终端里新建（或复用同 cwd 的）交互式会话，返回 session_id 与欢迎信息；需要用户看到执行过程时用它，普通一次性命令优先 command_run。命令用 terminal_send 发送", params(&[("cwd", "string", "可选，项目内的相对工作目录，空串表示项目根")]), blocking(move |args: String| { // 打开/复用终端会话
        let Some(root) = base.clone() else { return NO_PROJECT.to_string(); }; // 未选项目目录返回提示
        let cwd_text = arg(&args, "cwd"); // 目标工作目录
        let target = match resolve_in_root(&root, &cwd_text) { Ok(value) => value, Err(err) => { remember_tool_meta("terminal_open", json!({"ok": false, "error": err})); return format!("打开终端失败（{err}）"); } }; // 校验路径，失败写元数据并报错
        let actual = crate::agent::tools_details::common::normalized_dir_text(&target); // 归一化工作目录，匹配 session.cwd
        for session in terminal_sessions() { // 查找可复用的交互式 agent 会话
            if session.kind == "agent" && !session.persistent.load(Ordering::SeqCst) && session.cwd == actual && session_alive(&session) { // 同目录、非后台保活且存活则复用
                notify_terminal("open", json!({"session_id": session.id, "cwd": actual, "command": "", "reused": true})); // 广播打开事件
                remember_tool_meta("terminal_open", json!({"sessionId": session.id, "cwd": actual, "reused": true, "ok": true})); // 写元数据
                return json!({"session_id": session.id, "cwd": actual, "reused": true, "motd": ""}).to_string(); // 返回复用结果
            }
        }
        match crate::agent::tools_details::common::open_terminal(&root, &target) { // 无可复用会话则新建
            Ok(session) => { // 新建成功
                notify_terminal("open", json!({"session_id": session.id, "cwd": session.cwd, "command": "", "reused": false})); // 广播打开事件
                std::thread::sleep(Duration::from_millis(300)); // 等待欢迎信息
                let (text, _) = read_since(&session, 0); // 读取初始输出
                let motd = truncate_chars(&clean_terminal_output(&text), OUTPUT_LIMIT); // 清理并截断欢迎信息
                remember_tool_meta("terminal_open", json!({"sessionId": session.id, "cwd": session.cwd, "reused": false, "ok": true})); // 写元数据
                json!({"session_id": session.id, "cwd": session.cwd, "reused": false, "motd": motd}).to_string() // 返回结果
            }
            Err(err) => { remember_tool_meta("terminal_open", json!({"ok": false, "error": err})); format!("打开终端失败（{err}）") } // 新建失败
        }
    }));
    let base = root.clone(); // 克隆项目根供 send 使用
    let send = ToolDef::new("terminal_send", "向终端会话发送一段文本（submit 为 true 时补回车执行），输出静默后返回本次新增内容与 wait_reason(idle|timeout|session_exit|background)；不在白名单内的命令会先请求用户批准。本工具已带回本次输出，不要紧接着调 terminal_read。background=true 用于 npm run dev 等长驻服务：会话后台保活，只等一小段启动输出就返回", params(&[("session_id", "string", "terminal_open 返回的会话 id"), ("text", "string", "要发送的文本"), ("submit", "boolean", "可选，默认 true，为 false 时不补回车"), ("timeout", "integer", "可选，等待输出的秒数，0 表示用默认值"), ("background", "boolean", "可选，长驻服务设 true 后台保活")]), blocking(move |args: String| { // 向终端发送文本
        let Some(_root) = base.clone() else { return NO_PROJECT.to_string(); }; // 未选项目目录返回提示
        let value: Value = serde_json::from_str(&args).unwrap_or_else(|_| json!({})); // 解析参数
        let body = value.get("text").and_then(Value::as_str).unwrap_or_default().to_string(); // 发送文本
        if body.trim().is_empty() { return "文本不能为空".to_string(); } // 空文本拒绝
        let session_id = value.get("session_id").and_then(Value::as_str).unwrap_or_default().trim().to_string(); // 会话 id
        if session_id.is_empty() { return "缺少 session_id，请先调用 terminal_open".to_string(); } // 缺少 id
        let Some(session) = session_by_id(&session_id) else {
            remember_tool_meta("terminal_send", json!({"sessionId": session_id, "command": body, "ok": false, "error": "终端会话不存在"}));
            return format!("命令失败（终端会话不存在）：{session_id}");
        }; // 会话不存在
        if !session_alive(&session) { remember_tool_meta("terminal_send", json!({"sessionId": session_id, "command": body, "ok": false, "error": "终端会话不存在"})); return format!("命令失败（终端会话不存在）：{session_id}"); } // 会话已退出
        let submit = value.get("submit").and_then(Value::as_bool).unwrap_or(true); // 是否补回车
        let background = value.get("background").and_then(Value::as_bool).unwrap_or(false); // 是否后台保活
        let wait = value.get("timeout").and_then(Value::as_i64).unwrap_or(0); // 自定义等待秒数
        let cwd = session.cwd.clone(); // 工作目录
        let begin = Instant::now(); // 记录起始时间
        let (_, start) = read_since(&session, 0); // 记录当前输出偏移
        if background { // 后台服务模式
            session.persistent.store(true, Ordering::SeqCst); // 标记持久保活
            *session.command.lock() = body.clone(); // 记录命令
        }
        notify_terminal("send", json!({"session_id": session.id, "cwd": cwd, "command": if submit { body.clone() } else { String::new() }, "background": background})); // 广播发送事件
        let payload = if submit { format!("{body}\r") } else { body.clone() }; // submit 时补回车
        if let Err(err) = write_session(&session, &payload) { // 写入终端
            remember_tool_meta("terminal_send", json!({"sessionId": session.id, "command": body, "cwd": cwd, "ok": false, "error": err})); // 写失败元数据
            return format!("命令失败（写入终端失败）：{err}"); // 返回写入错误
        }
        if background { // 后台模式：短暂等待后返回
            std::thread::sleep(Duration::from_millis(1500)); // 等待启动输出
            let (text, _) = read_since(&session, start); // 读取增量
            let output = truncate_chars(&clean_terminal_output(&text), OUTPUT_LIMIT); // 清理并截断
            remember_tool_meta("terminal_send", json!({"sessionId": session.id, "command": body, "cwd": cwd, "output": output, "waitReason": "background", "background": true, "ok": true, "durationMs": begin.elapsed().as_millis() as i64})); // 写元数据
            return json!({"session_id": session.id, "cwd": cwd, "output": output, "wait_reason": "background", "background": true, "persistent": true, "alive": session_alive(&session), "hint": "服务已在后台保活，用 terminal_read(since) 读增量输出，用 terminal_list 查存活"}).to_string(); // 返回后台结果
        }
        let limit = if wait > 0 { wait } else { timeout_seconds.max(1) }; // 确定等待上限
        let deadline = Instant::now() + Duration::from_secs(limit as u64); // 超时截止
        let mut last_len = start; // 上次输出长度
        let mut last_change = Instant::now(); // 上次变化时间
        let mut timed_out = false; // 超时标记
        let mut exited = false; // 会话退出标记
        loop { // 等待输出静默
            if !session_alive(&session) { exited = true; break; } // 会话退出则结束
            let (_, total) = read_since(&session, start); // 读当前输出长度
            if total != last_len { last_len = total; last_change = Instant::now(); } // 有新输出则更新时间
            if last_change.elapsed().as_secs_f64() >= QUIET_SECONDS { break; } // 静默超时结束
            if Instant::now() >= deadline { timed_out = true; break; } // 整体超时结束
            std::thread::sleep(Duration::from_millis(50)); // 轮询间隔
        }
        let (text, _) = read_since(&session, start); // 读取本次新增输出
        let output = truncate_chars(&clean_terminal_output(&text), OUTPUT_LIMIT); // 清理并截断
        let reason = if exited { "session_exit" } else if timed_out { "timeout" } else { "idle" }; // 等待原因
        remember_tool_meta("terminal_send", json!({"sessionId": session.id, "command": body, "cwd": cwd, "output": output, "waitReason": reason, "timedOut": timed_out, "exited": exited, "ok": !timed_out, "durationMs": begin.elapsed().as_millis() as i64})); // 写元数据
        json!({"session_id": session.id, "cwd": cwd, "output": output, "wait_reason": reason, "timed_out": timed_out, "exit": exited}).to_string() // 返回结果
    })).with_command("text"); // 标记命令字段供审批
    let base = root.clone(); // 克隆项目根供 read 使用
    let read = ToolDef::new("terminal_read", "读取终端会话回滚缓冲自 since 起的新增输出（不发送任何内容）；仅在确实需要回看历史时调用，不要紧跟 terminal_send。返回 text/total/truncated", params(&[("session_id", "string", "终端会话 id"), ("since", "integer", "可选，起始字节偏移，默认 0")]), blocking(move |args: String| { // 读取终端增量
        if base.is_none() { return NO_PROJECT.to_string(); } // 未选项目目录返回提示
        let value: Value = serde_json::from_str(&args).unwrap_or_else(|_| json!({})); // 解析参数
        let session_id = value.get("session_id").and_then(Value::as_str).unwrap_or_default().trim().to_string(); // 会话 id
        if session_id.is_empty() { return "缺少 session_id，请先调用 terminal_open".to_string(); } // 缺少 id
        let Some(session) = session_by_id(&session_id) else { return format!("读取失败（终端会话不存在）：{session_id}"); }; // 会话不存在
        let since = value.get("since").and_then(Value::as_i64).unwrap_or(0).max(0) as usize; // 起始偏移
        let (text, total) = read_since(&session, since); // 读取增量
        let cleaned = clean_terminal_output(&text); // 清理 ANSI
        json!({"session_id": session_id, "cwd": session.cwd, "text": truncate_chars(&cleaned, OUTPUT_LIMIT), "since": since, "total": total, "truncated": cleaned.chars().count() > OUTPUT_LIMIT}).to_string() // 返回结果
    }));
    let base = root; // 移动项目根供 list 使用
    let list = ToolDef::new("terminal_list", "列出本项目当前存活的智能体终端会话（session_id/cwd/alive/persistent/command）；后台服务会带 persistent=true", params(&[]), blocking(move |args: String| { // 列出本项目终端会话
        let _ = args; // 忽略参数
        let Some(root) = base.clone() else { return NO_PROJECT.to_string(); }; // 未选项目目录返回提示
        let marker = root_text(&root); // 项目根标识
        let items: Vec<Value> = terminal_sessions().into_iter().filter(|session: &std::sync::Arc<TerminalSession>| session.root == marker).map(|session| json!({"session_id": session.id, "cwd": session.cwd, "alive": session_alive(&session), "persistent": session.persistent.load(Ordering::SeqCst), "command": session.command.lock().clone()})).collect(); // 过滤本项目会话并投影字段
        if items.is_empty() { "（当前没有智能体终端会话，先用 terminal_open 创建）".to_string() } else { serde_json::to_string(&items).unwrap_or_else(|_| "（当前没有智能体终端会话）".to_string()) } // 空列表给占位
    }));
    vec![open, send, read, list] // 返回四个工具
}
