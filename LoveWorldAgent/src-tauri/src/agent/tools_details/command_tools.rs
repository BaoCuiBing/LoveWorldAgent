use std::path::{Path, PathBuf}; // 命令执行工具：通过 terminal_router 的 agent ConPTY 会话执行命令，右侧终端实时可见，结果写旁路元数据
use std::sync::Arc; // 共享终端会话引用
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH}; // 超时、计时与唯一标记时间戳
use once_cell::sync::Lazy; // 进程内串行锁惰性初始化
use parking_lot::Mutex; // 同一时刻仅允许一条 command_run 在交互式 ConPTY 中顺序执行，避免输出交错
use serde_json::{json, Value}; // JSON 构造与动态值
use crate::agent::tools::{params, blocking, ToolDef}; // 工具定义与同步包装
use crate::agent::tools_details::common::{clean_terminal_output, notify_terminal, open_terminal, read_since, remember_tool_meta, session_alive, terminal_sessions, truncate_chars, write_session, TerminalSession, NO_PROJECT, OUTPUT_LIMIT}; // 终端会话与元数据辅助
static CMD_PTY_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(())); // 串行化 command_run 对 agent 终端的写入与采集
fn canonical_cwd(root: &Path) -> String { // 归一化项目根目录字符串，与 terminal_router::resolve_cwd 对齐（无 \\?\ 前缀）
    crate::agent::tools_details::common::normalized_dir_text(root)
}
fn pick_agent_session(root: &Path) -> Result<(Arc<TerminalSession>, bool), String> { // 查找并复用存活的非后台 agent 会话，无则新建
    let actual = canonical_cwd(root); // 与 TerminalSession::cwd 格式完全对齐（无 \\?\）
    for session in terminal_sessions() { // 优先复用同目录交互式 agent 会话
        if session.kind == "agent" && !session.persistent.load(std::sync::atomic::Ordering::SeqCst) && session.cwd == actual && session_alive(&session) {
            return Ok((session, true));
        }
    }
    let session = open_terminal(root, root)?; // 新建 agent ConPTY 会话
    std::thread::sleep(Duration::from_millis(250)); // 等待 cmd.exe 初始提示符就绪
    Ok((session, false)) // 返回新会话与复用标记
}
fn has_bash_heredoc(text: &str) -> bool { // 检测 Windows cmd.exe 不支持的 bash heredoc（如 <<EOF / <<PYEOF）
    let mut lines = text.lines();
    let Some(first) = lines.next() else { return false; };
    let Some(idx) = first.find("<<") else { return false; };
    let after = first[idx + 2..].trim_start_matches('-').trim_start();
    let tag: String = after.trim_matches(|c| c == '\'' || c == '"').chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
    !tag.is_empty() && lines.any(|line| line.trim() == tag)
}
fn extract_between_markers(cleaned: &str, begin_tag: &str, end_tag: &str) -> Option<(i32, String)> { // 从清理后的 PTY 输出中截取 begin/end 标记之间的纯净命令输出与退出码
    let after_echo = cleaned.find("errorlevel%").and_then(|pos| cleaned[pos..].find('\n').map(|nl| pos + nl + 1)).unwrap_or(0); // 跳过 ConPTY 回显的命令行本身
    let slice = &cleaned[after_echo..]; // 仅检查实际执行阶段输出
    let begin_line = format!("\n{begin_tag}\n"); // 独立行起始标记
    let start_pos = if let Some(rest) = slice.strip_prefix(&format!("{begin_tag}\n")) { 0usize.saturating_add(slice.len() - rest.len()) } else { slice.find(&begin_line).map(|p| p + begin_line.len())? }; // 定位正文起点
    let body_tail = &slice[start_pos..]; // 起点之后文本
    let end_prefix = format!("{end_tag}:"); // 结束标记前缀
    for (idx, line_start) in std::iter::once(0).chain(body_tail.match_indices('\n').map(|(i, _)| i + 1)).enumerate() { // 逐行查找结尾标记
        let _ = idx; // 仅使用偏移
        let line = body_tail[line_start..].split('\n').next().unwrap_or(""); // 取当前行
        let trimmed = line.trim(); // 去除空白
        if let Some(code_str) = trimmed.strip_prefix(&end_prefix) { // 命中 __LWA_E_xxx__:<exit_code>
            if let Ok(code) = code_str.trim().parse::<i32>() { // 解析整数退出码
                let output = body_tail[..line_start].trim().to_string(); // 截取纯净命令输出
                return Some((code, output)); // 返回退出码与输出
            }
        }
    }
    None // 尚未等到完整结束标记
}
fn run_command_in_pty(root: &Path, text: &str, timeout_seconds: i64) -> (i32, String, bool, String) { // 在 agent ConPTY 中执行命令并返回 (退出码, 输出, 是否超时, session_id)
    let _guard = CMD_PTY_LOCK.lock(); // 串行锁防止并发命令写乱同一 PTY
    let (session, reused) = match pick_agent_session(root) { Ok(value) => value, Err(err) => return (127, format!("命令失败（无法打开终端会话）：{err}"), false, String::new()) }; // 获取或新建终端会话
    let sid = session.id.clone(); // 记录会话 id 供前端绑定
    *session.command.lock() = text.to_string(); // 更新当前执行命令展示
    notify_terminal("open", json!({"session_id": sid, "cwd": session.cwd, "command": text, "reused": reused})); // 广播打开/聚焦右侧终端
    let (_, start) = read_since(&session, 0); // 记录写入前缓冲偏移
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos(); // 纳秒时间戳保证标记唯一
    let begin_tag = format!("__LWA_B_{stamp}__"); // 起始标记
    let end_tag = format!("__LWA_E_{stamp}__"); // 结束标记
    let temp_cmd = if text.contains('\n') { // 多行 cmd 脚本通过临时 .cmd 文件执行，避免 \n 替换 & 破坏逻辑
        let batch_path = std::env::temp_dir().join(format!("lwa_cmd_{stamp}.cmd"));
        let body = format!("@echo off\r\n{}\r\n", text.replace("\r\n", "\n").replace('\n', "\r\n"));
        if let Err(err) = std::fs::write(&batch_path, body) {
            return (127, format!("命令失败（创建临时脚本失败）：{err}"), false, sid);
        }
        Some(batch_path)
    } else { None };
    let exec_expr = match &temp_cmd {
        Some(batch) => format!("call \"{}\"", batch.to_string_lossy()),
        None => text.replace('\r', ""),
    };
    let wrapped = format!("echo {begin_tag}& ({exec_expr}) & call echo {end_tag}:^%errorlevel^%\r\n"); // 包裹首尾标记并延迟展开 errorlevel
    if let Err(err) = write_session(&session, &wrapped) {
        if let Some(batch) = &temp_cmd { let _ = std::fs::remove_file(batch); }
        return (127, format!("命令失败（写入终端失败）：{err}"), false, sid);
    } // 写入 ConPTY 失败返回 127
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds.max(1) as u64); // 计算超时时间点
    let mut timed_out = false; // 超时标志
    let (code, body_raw) = loop { // 轮询增量输出直到命中结束标记、会话退出或超时
        let (raw, _) = read_since(&session, start); // 读取写入以来的增量字节
        let cleaned = clean_terminal_output(&raw); // 剥离 ANSI 控制序列
        if let Some(hit) = extract_between_markers(&cleaned, &begin_tag, &end_tag) { break hit; } // 命中完整输出与退出码
        if !session_alive(&session) { break (1, cleaned.trim().to_string()); } // 会话意外退出
        if Instant::now() >= deadline { timed_out = true; let _ = write_session(&session, "\x03\r\n"); std::thread::sleep(Duration::from_millis(200)); let (raw_tail, _) = read_since(&session, start); break (124, clean_terminal_output(&raw_tail).trim().to_string()); } // 超时发送 Ctrl+C 并回收已有输出
        std::thread::sleep(Duration::from_millis(50)); // 50ms 轮询间隔
    };
    if let Some(batch) = &temp_cmd { let _ = std::fs::remove_file(batch); } // 清理临时脚本
    *session.command.lock() = String::new(); // 清空当前活动命令
    let body = if body_raw.is_empty() { "（无输出）".to_string() } else { truncate_chars(&body_raw, OUTPUT_LIMIT) }; // 空输出占位或截断超长文本
    (code, body, timed_out, sid) // 返回完整四元组
}
pub fn build_command_tools(root: Option<PathBuf>, timeout_seconds: i64) -> Vec<ToolDef> { // 构造命令执行工具
    let base = root; // 项目根目录
    let run = ToolDef::new("command_run", "在项目目录的内置 Windows cmd.exe 终端会话中执行命令并返回输出；右侧终端实时同步并自动复用持久会话。不支持 bash heredoc(<<EOF)，多行 Python 请先 file_write 写 .py 再 python 执行；长驻服务请用 terminal_send(background=true)", params(&[("command", "string", "要执行的 Windows cmd 命令行")]), blocking(move |args: String| { // 命令工具定义与处理器
        let Some(root) = base.clone() else { return NO_PROJECT.to_string(); }; // 未选项目目录返回提示
        let value: Value = serde_json::from_str(&args).unwrap_or_else(|_| json!({})); // 解析参数，非法按空对象
        let text = value.get("command").and_then(Value::as_str).unwrap_or_default().trim().to_string(); // 命令文本
        if text.is_empty() { return "命令不能为空".to_string(); } // 空命令拒绝
        if has_bash_heredoc(&text) {
            let msg = "命令失败（Windows cmd.exe 不支持 bash heredoc <<EOF 语法；请先用 file_write 写入 .py 脚本再执行 python xxx.py，或改用单行 python -c）".to_string();
            remember_tool_meta("command_run", json!({"command": text, "cwd": canonical_cwd(&root), "exitCode": 2, "ok": false, "output": msg, "durationMs": 0, "error": "unsupported_heredoc"}));
            return msg;
        }
        let begin = Instant::now(); // 记录起始时间
        let (code, body, timed_out, sid) = run_command_in_pty(&root, &text, timeout_seconds); // 在 agent ConPTY 中执行命令
        let ok = code == 0 && !timed_out; // 成功需退出码零且未超时
        remember_tool_meta("command_run", json!({"command": text, "cwd": canonical_cwd(&root), "sessionId": sid, "exitCode": code, "ok": ok, "output": body, "durationMs": begin.elapsed().as_millis() as i64, "error": if timed_out { "timeout".to_string() } else { String::new() }})); // 写回旁路元数据供卡片与终端面板关联
        if timed_out { return format!("命令失败（超时 {timeout_seconds} 秒）：{text}\n{body}"); } // 超时回报
        if code != 0 { return format!("命令失败（退出码 {code}）：{body}"); } // 非零退出码回报
        body // 成功返回输出
    })).with_command("command"); // 标记命令字段供审批判定
    vec![run] // 返回工具列表
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extract_between_markers_parses_output_and_exit_code() {
        let cleaned = "E:\\proj>echo __LWA_B_1__& (echo ok) & call echo __LWA_E_1__:^%errorlevel^%\n__LWA_B_1__\nok\n__LWA_E_1__:0\nE:\\proj>";
        let hit = extract_between_markers(cleaned, "__LWA_B_1__", "__LWA_E_1__").expect("must parse marker");
        assert_eq!(hit.0, 0);
        assert_eq!(hit.1, "ok");
        let cleaned_fail = "E:\\proj>echo __LWA_B_2__& (cmd /c exit 7) & call echo __LWA_E_2__:^%errorlevel^%\n__LWA_B_2__\nerr line\n__LWA_E_2__:7\nE:\\proj>";
        let hit_fail = extract_between_markers(cleaned_fail, "__LWA_B_2__", "__LWA_E_2__").expect("must parse non-zero exit code");
        assert_eq!(hit_fail.0, 7);
        assert_eq!(hit_fail.1, "err line");
    }
    #[test]
    fn heredoc_detection_catches_bash_only_syntax() {
        assert!(has_bash_heredoc("python - <<PYEOF\nprint(1)\nPYEOF"));
        assert!(has_bash_heredoc("cat << 'EOF'\nhello\nEOF"));
        assert!(!has_bash_heredoc("python -c \"print(1 << 2)\""));
        assert!(!has_bash_heredoc("cd frontend\nnpm run build"));
    }
}
