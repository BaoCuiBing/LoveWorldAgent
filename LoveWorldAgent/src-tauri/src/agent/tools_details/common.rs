use std::collections::HashMap; // 工具公共层：常量、路径门禁、输出解码、调用元数据、提问/待办状态、终端会话与事件回调
use std::hash::{Hash, Hasher}; // 问题指纹计算
use std::path::{Path, PathBuf}; // 路径处理
use std::sync::Arc; // 会话与回调共享
use once_cell::sync::Lazy; // 全局惰性状态
use parking_lot::Mutex; // 互斥锁
use serde_json::{json, Value}; // JSON 构造
use crate::utils::logger; // 日志输出
pub const READ_LIMIT: usize = 200 * 1024; // file_read 单次上限 200KB
pub const WRITE_LIMIT: usize = 1024 * 1024; // file_write 单次上限 1MB
pub const OUTPUT_LIMIT: usize = 4000; // 命令/终端输出截断长度
pub const TOOL_LOG_LIMIT: usize = 400; // 调用日志摘要长度
pub const NO_PROJECT: &str = "请先选择项目文件夹"; // 未选项目目录的统一提示
pub const QUIET_SECONDS: f64 = 0.8; // 终端输出静默判定的时长
pub const HUNK_LIMIT: usize = 400; // diff 逐行明细上限
pub const TODO_LIMIT: usize = 50; // todo_write 单次最大条数
pub const TODO_STATUSES: [&str; 3] = ["pending", "in_progress", "completed"]; // 合法待办状态
pub const FORBIDDEN_DIRS: [&str; 6] = [".venv", "node_modules", "dist", "webdist", ".git", "__pycache__"]; // 禁止访问的目录
pub const FORBIDDEN_FILES: [&str; 6] = ["src-tauri/static/config.json", "src-tauri/static/.loveword/agent_config.json", "src-tauri/static/.loveword/agent_permission.json", "src-tauri/static/.loveword/agent_mcp.json", "src-tauri/static/.loveword/agent_browser.json", "src-tauri/static/.loveword/agent_computer_use.json"]; // 禁止访问的配置文件
pub fn clean_terminal_output(text: &str) -> String { // 去除终端输出中的 ANSI 转义序列
    let chars: Vec<char> = text.chars().collect(); // 字符数组
    let mut out = String::new(); // 清理结果
    let mut index = 0usize; // 遍历位置
    while index < chars.len() { // 逐字符扫描
        let ch = chars[index]; // 当前字符
        if ch == '\u{1b}' { // 遇到 ESC
            if index + 1 < chars.len() && chars[index + 1] == ']' { // OSC 序列
                index += 2; // 跳过 ESC ]
                while index < chars.len() && chars[index] != '\u{7}' { if chars[index] == '\u{1b}' && index + 1 < chars.len() && chars[index + 1] == '\\' { index += 2; break; } index += 1; } // 读到 BEL 或 ESC \
                index += 1; // 跳过终止符
                continue; // 继续扫描
            }
            if index + 1 < chars.len() && chars[index + 1] == '[' { // CSI 序列
                index += 2; // 跳过 ESC [
                while index < chars.len() && !(chars[index] as u32 >= 0x40 && chars[index] as u32 <= 0x7e) { index += 1; } // 读到最终字节
                index += 1; // 跳过最终字节
                continue; // 继续扫描
            }
            index += 1; // 其他 ESC 序列只跳过 ESC
            continue; // 继续扫描
        }
        if ch != '\r' { out.push(ch); } // 去掉回车符
        index += 1; // 前进
    }
    out // 返回清理结果
}
pub fn decode_output(raw: &[u8]) -> String { // 字节输出解码为字符串
    if raw.is_empty() { return String::new(); } // 空输入返回空串
    match String::from_utf8(raw.to_vec()) { // 优先按 UTF-8 解码
        Ok(text) => text, // 合法 UTF-8
        Err(_) => String::from_utf8_lossy(raw).to_string(), // 非法则损失性解码
    }
}
pub fn truncate_chars(text: &str, limit: usize) -> String { // 按字符数截断
    text.chars().take(limit).collect() // 取前 limit 个字符
}
pub fn squash(text: &str) -> String { // 压缩空白为单空格
    text.split_whitespace().collect::<Vec<&str>>().join(" ") // 按空白切分拼接
}
static TOOL_META: Lazy<Mutex<HashMap<String, Value>>> = Lazy::new(|| Mutex::new(HashMap::new())); // 工具旁路元数据存储
static TOOL_IMAGES: Lazy<Mutex<HashMap<String, Vec<(String, String)>>>> = Lazy::new(|| Mutex::new(HashMap::new())); // 工具旁路图片（mime, base64）
pub fn remember_tool_meta(tool: &str, data: Value) { // 写入工具元数据
    TOOL_META.lock().insert(tool.to_string(), data); // 按工具名保存
}
pub fn take_tool_meta(tool: &str) -> Value { // 取走并移除工具元数据
    TOOL_META.lock().remove(tool).unwrap_or_else(|| json!({})) // 无记录返回空对象
}
pub fn remember_tool_images(tool: &str, images: Vec<(String, String)>) { // 写入工具图片（覆盖，避免陈旧）
    TOOL_IMAGES.lock().insert(tool.to_string(), images); // 按工具名保存
}
pub fn take_tool_images(tool: &str) -> Vec<(String, String)> { // 取走并移除工具图片
    TOOL_IMAGES.lock().remove(tool).unwrap_or_default() // 无记录返回空
}
pub fn clear_tool_meta() { // 清空全部工具元数据
    TOOL_META.lock().clear(); // 清空映射
    TOOL_IMAGES.lock().clear(); // 同步清空图片
}
static ANSWERS: Lazy<Mutex<HashMap<String, String>>> = Lazy::new(|| Mutex::new(HashMap::new())); // 待取回的用户答案
pub fn answer_key(questions: &str) -> String { // 按问题文本生成指纹
    let mut hasher = std::collections::hash_map::DefaultHasher::new(); // 哈希器
    questions.hash(&mut hasher); // 写入问题文本
    format!("{:016x}", hasher.finish()) // 格式化为 16 位十六进制
}
pub fn set_pending_answers(questions: &str, answers: &Value) { // 暂存用户答案
    ANSWERS.lock().insert(answer_key(questions), serde_json::to_string(answers).unwrap_or_else(|_| "[]".to_string())); // 按指纹保存 JSON
}
pub fn take_answer(questions: &str) -> String { // 按问题指纹取回答案
    ANSWERS.lock().remove(&answer_key(questions)).unwrap_or_default() // 移除并返回
}
static TODOS: Lazy<Mutex<Vec<Value>>> = Lazy::new(|| Mutex::new(Vec::new())); // 模块级待办清单
pub fn set_todos(rows: Vec<Value>) -> Vec<Value> { // 覆盖待办清单
    let mut guard = TODOS.lock(); // 加锁
    *guard = rows; // 替换内容
    guard.clone() // 返回副本
}
pub fn latest_todos() -> Vec<Value> { // 读取当前待办清单
    TODOS.lock().clone() // 克隆返回
}
pub use crate::rouer::terminal_router::TerminalSession; // 终端会话类型由 terminal_router 统一托管，智能体工具与右侧终端共用同一会话池
pub fn normalized_dir_text(path: &Path) -> String { // 规范化目录路径文本，消除 Windows \\?\ 前缀差异
    crate::config::strip_verbatim(std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())).to_string_lossy().to_string() // 去除 \\?\ 前缀后转字符串
}
pub fn open_terminal(root: &Path, cwd: &Path) -> Result<Arc<TerminalSession>, String> { // 为智能体新建终端会话
    let target = normalized_dir_text(cwd); // 归一化工作目录
    let marker = normalized_dir_text(root); // 归一化项目根标识，确保与 terminal_list 一致
    crate::rouer::terminal_router::manager().open(120, 30, Some(target.as_str()), "agent", false, &marker) // 120x30，归属 agent
}
pub fn session_by_id(id: &str) -> Option<Arc<TerminalSession>> { // 按 id 查终端会话
    crate::rouer::terminal_router::manager().get(id) // 委托管理器
}
pub fn terminal_sessions() -> Vec<Arc<TerminalSession>> { // 列出全部终端会话
    crate::rouer::terminal_router::manager().all() // 委托管理器
}
pub fn session_alive(session: &TerminalSession) -> bool { // 判断会话是否仍存活
    session.run_alive() // 委托会话
}
pub fn write_session(session: &TerminalSession, text: &str) -> Result<(), String> { // 向终端写入文本
    crate::rouer::terminal_router::manager().write(&session.id, text) // 委托管理器
}
pub fn read_since(session: &TerminalSession, since: usize) -> (String, usize) { // 读取 since 之后的增量输出
    crate::rouer::terminal_router::read_session(session, since) // 委托读取
}
pub type TerminalHook = Arc<dyn Fn(&str, Value) + Send + Sync>; // 终端事件回调类型
static HOOKS: Lazy<Mutex<Vec<(u64, TerminalHook)>>> = Lazy::new(|| Mutex::new(Vec::new())); // 回调注册表
static HOOK_SEQ: Lazy<Mutex<u64>> = Lazy::new(|| Mutex::new(0)); // 回调 id 计数器
pub fn add_terminal_hook(hook: TerminalHook) -> u64 { // 注册终端事件回调
    let mut seq = HOOK_SEQ.lock(); // 加锁计数
    *seq += 1; // 自增 id
    HOOKS.lock().push((*seq, hook)); // 登记回调
    *seq // 返回 id
}
pub fn remove_terminal_hook(id: u64) { // 移除终端事件回调
    HOOKS.lock().retain(|(key, _)| *key != id); // 按 id 过滤
}
pub fn notify_terminal(event: &str, payload: Value) { // 向所有回调广播终端事件
    let hooks: Vec<TerminalHook> = HOOKS.lock().iter().map(|(_, hook)| hook.clone()).collect(); // 克隆回调列表
    for hook in hooks { // 逐个调用
        if let Err(err) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| hook(event, payload.clone()))) { // 捕获回调 panic
            logger::debug("tools", &format!("终端事件回调异常: {event} {err:?}")); // 记录异常
        }
    }
}
fn normalize_target(root: &Path, raw: &str) -> Result<PathBuf, String> { // 校验并规范化项目内路径
    let text = raw.trim().replace('\\', "/"); // 统一斜杠
    let text = text.trim_start_matches('/').to_string(); // 去前导斜杠
    let base = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf()); // 规范化项目根
    if text.is_empty() || text == "." { return Ok(base); } // 空路径返回项目根
    let joined = base.join(&text); // 拼接路径
    let target = if joined.exists() { std::fs::canonicalize(&joined).map_err(|err| err.to_string())? } else { // 已存在则规范化
        let parent = joined.parent().map(|path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())).unwrap_or_else(|| base.clone()); // 不存在则规范化父目录
        parent.join(joined.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default()) // 拼接文件名
    };
    if target != base && !target.starts_with(&base) { return Err(format!("路径越界，仅允许项目内相对路径: {text}")); } // 越界拒绝
    let rel = target.strip_prefix(&base).map(|path| path.to_string_lossy().replace('\\', "/")).unwrap_or_default(); // 取相对路径
    if rel.split('/').any(|part| FORBIDDEN_DIRS.contains(&part)) { return Err(format!("禁止访问该目录: {text}")); } // 禁访目录拒绝
    let lower = rel.to_lowercase(); // 小写化便于比较
    if FORBIDDEN_FILES.contains(&lower.as_str()) { return Err("禁止访问配置文件".to_string()); } // 禁访文件拒绝
    if lower.contains(".loveword/agents/") { return Err("禁止访问智能体配置文件".to_string()); } // 子智能体配置拒绝
    Ok(target) // 返回合法路径
}
pub fn resolve_in_root(root: &Path, raw: &str) -> Result<PathBuf, String> { // 对外暴露路径门禁
    normalize_target(root, raw) // 委托规范化
}
fn lcs_rows(old: &[String], new: &[String]) -> Vec<(String, usize, usize, usize, usize)> { // LCS 算法生成行级 diff 操作
    let rows = old.len(); // 旧文件行数
    let cols = new.len(); // 新文件行数
    let mut table = vec![vec![0usize; cols + 1]; rows + 1]; // DP 表
    for i in (0..rows).rev() { // 倒序遍历旧行
        for j in (0..cols).rev() { // 倒序遍历新行
            table[i][j] = if old[i] == new[j] { table[i + 1][j + 1] + 1 } else { table[i + 1][j].max(table[i][j + 1]) }; // LCS 递推
        }
    }
    let mut ops: Vec<(String, usize, usize, usize, usize)> = Vec::new(); // diff 操作序列
    let (mut i, mut j) = (0usize, 0usize); // 双指针
    while i < rows && j < cols { // 遍历 DP 表回溯
        if old[i] == new[j] { ops.push(("equal".to_string(), i, i + 1, j, j + 1)); i += 1; j += 1; } // 相等行
        else if table[i + 1][j] >= table[i][j + 1] { ops.push(("delete".to_string(), i, i + 1, j, j)); i += 1; } // 删除行
        else { ops.push(("insert".to_string(), i, i, j, j + 1)); j += 1; } // 新增行
    }
    while i < rows { ops.push(("delete".to_string(), i, i + 1, j, j)); i += 1; } // 剩余旧行全删
    while j < cols { ops.push(("insert".to_string(), i, i, j, j + 1)); j += 1; } // 剩余新行全增
    ops // 返回操作序列
}
pub fn diff_hunks(before: &str, after: &str) -> (usize, usize, Vec<Value>) { // 计算逐行 diff，返回新增数/删除数/明细
    let old: Vec<String> = before.split('\n').map(str::to_string).collect(); // 旧文本按行切
    let new: Vec<String> = after.split('\n').map(str::to_string).collect(); // 新文本按行切
    if old.len() > 2000 || new.len() > 2000 { // 文件过大时省略明细
        let additions = new.len(); // 新增数按新行数
        let deletions = old.len(); // 删除数按旧行数
        let rows = if old.len() >= new.len() { deletions } else { additions }; // 明细行数取较大者
        return (additions, deletions, (0..rows.min(HUNK_LIMIT)).map(|index| json!({"type": "context", "content": "（文件较大，逐行明细已省略）", "oldLineNo": index + 1, "newLineNo": index + 1})).collect()); // 返回占位明细
    }
    let mut additions = 0usize; // 新增计数
    let mut deletions = 0usize; // 删除计数
    let mut out: Vec<Value> = Vec::new(); // 明细列表
    for (kind, i1, i2, j1, j2) in lcs_rows(&old, &new) { // 遍历 diff 操作
        match kind.as_str() { // 按操作类型分派
            "equal" => { // 未变化行
                for offset in 0..(i2 - i1) { // 逐行
                    if out.len() >= HUNK_LIMIT { break; } // 达上限停止
                    out.push(json!({"type": "context", "content": old[i1 + offset], "oldLineNo": i1 + offset + 1, "newLineNo": j1 + offset + 1})); // 记录上下文行
                }
            }
            "delete" => { // 删除行
                for offset in 0..(i2 - i1) { // 逐行
                    deletions += 1; // 计数
                    if out.len() < HUNK_LIMIT { out.push(json!({"type": "delete", "content": old[i1 + offset], "oldLineNo": i1 + offset + 1, "newLineNo": Value::Null})); } // 记录删除行
                }
            }
            _ => { // 新增行
                for offset in 0..(j2 - j1) { // 逐行
                    additions += 1; // 计数
                    if out.len() < HUNK_LIMIT { out.push(json!({"type": "add", "content": new[j1 + offset], "oldLineNo": Value::Null, "newLineNo": j1 + offset + 1})); } // 记录新增行
                }
            }
        }
        if out.len() >= HUNK_LIMIT { break; } // 达上限停止
    }
    (additions, deletions, out) // 返回统计与明细
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forbidden_files_blocks_agent_computer_use_config() {
        assert!(FORBIDDEN_FILES.contains(&"src-tauri/static/.loveword/agent_computer_use.json"));
        let root = std::env::temp_dir().join(format!("lwa_common_tools_{}", std::process::id()));
        let _ = std::fs::create_dir_all(root.join("src-tauri/static/.loveword"));
        let err = resolve_in_root(&root, "src-tauri/static/.loveword/agent_computer_use.json").unwrap_err();
        assert!(err.contains("禁止访问"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
