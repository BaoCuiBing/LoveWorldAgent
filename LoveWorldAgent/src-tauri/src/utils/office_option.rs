use serde_json::{json, Value}; // Office 工具 sidecar 管道客户端（PeekNamedPipe 非死锁）、三族独立自动截图配置与步骤日志
use std::path::{Path, PathBuf}; // 路径
use std::process::{Child, Command}; // sidecar 进程
use std::sync::mpsc::RecvTimeoutError; // 超时错误
use std::sync::{Mutex, MutexGuard, OnceLock}; // 全局单例
use std::time::{Duration, Instant}; // 超时与耗时
use crate::utils::sidecar_pipe::{self, extract_frame_screenshot, kill_sidecar_exe, PipeChannel, StepLog}; // 通用管道、进程清理、步骤记录与截图助手
use crate::{config, logger}; // 配置与日志
pub const PIPE_NAME: &str = "loveworld_office_option"; // Office 工具管道名（三族共用）
const MAX_FRAME: usize = 8192; // 单帧读取上限
const MAX_CONSECUTIVE_TIMEOUTS: u32 = 2; // 连续超时重启阈值
struct Session { // 会话状态
    child: Child, // sidecar 进程
    pipe: PipeChannel, // 非死锁管道通道
    next_id: u64, // 请求 id 序列
    consecutive_timeouts: u32, // 连续超时计数
}
static SESSION: OnceLock<Mutex<Option<Session>>> = OnceLock::new(); // 全局会话
static ENABLED: OnceLock<Mutex<(bool, bool, bool)>> = OnceLock::new(); // 三族开关（word/ppt/excel）
static AUTO_SCREENSHOT_CACHE: OnceLock<Mutex<Option<(bool, bool, bool)>>> = OnceLock::new(); // 三族独立自动截图开关缓存
static STEPS: StepLog = StepLog::new(); // 操作步骤环形日志
fn session() -> MutexGuard<'static, Option<Session>> { SESSION.get_or_init(|| Mutex::new(None)).lock().unwrap_or_else(|err| err.into_inner()) } // 取全局锁
fn enabled_guard() -> MutexGuard<'static, (bool, bool, bool)> { ENABLED.get_or_init(|| Mutex::new((true, true, true))).lock().unwrap_or_else(|err| err.into_inner()) } // 开关锁
fn auto_cache() -> &'static Mutex<Option<(bool, bool, bool)>> { AUTO_SCREENSHOT_CACHE.get_or_init(|| Mutex::new(None)) } // 自动截图缓存锁
pub fn family_name(cmd: &str) -> &'static str { if cmd.starts_with("ppt_") { "ppt" } else if cmd.starts_with("excel_") { "excel" } else { "word" } } // 命令归族文本
pub fn set_enabled(family: &str, on: bool) { // 按族启停（关闭时不强杀其他族）
    let mut state = enabled_guard(); // 取锁
    match family { "word" => state.0 = on, "ppt" => state.1 = on, "excel" => state.2 = on, _ => {} } // 写入
    let any = state.0 || state.1 || state.2; // 是否任一族启用
    drop(state); // 释放锁
    if !any { stop(); } // 全部关闭才停止
}
fn family_of(cmd: &str) -> u8 { if cmd.starts_with("ppt_") { 1 } else if cmd.starts_with("excel_") { 2 } else { 0 } } // 族编号
pub fn family_enabled(cmd: &str) -> bool { // 当前工具所属族是否启用
    let state = enabled_guard(); // 开关
    match family_of(cmd) { 1 => state.1, 2 => state.2, _ => state.0 } // 分派
}
fn parse_office_json(writable: &Path, bundled: &Path) -> Value { // 读取可写或随包 Office 配置
    let path = if writable.exists() { Some(writable) } else if bundled.exists() { Some(bundled) } else { None };
    config::read_json(path)
}
pub fn read_office_config_in(writable: &Path, bundled: &Path) -> (i64, (bool, bool, bool)) { // 解析备份保留数与三族独立自动截图开关（默认 5 与全 true）
    let data = parse_office_json(writable, bundled);
    let keep = data.get("backup").and_then(Value::as_object).and_then(|map| map.get("keep")).and_then(Value::as_i64).unwrap_or(5).clamp(1, 30);
    let auto_obj = data.get("auto_screenshot").and_then(Value::as_object);
    let flag = |key: &str| auto_obj.and_then(|map| map.get(key)).map(sidecar_pipe::truthy).unwrap_or(true);
    (keep, (flag("word"), flag("ppt"), flag("excel")))
}
pub fn write_office_config_in(writable: &Path, bundled: &Path, family: &str, backup_keep: Option<i64>, auto_screenshot: Option<bool>) -> anyhow::Result<(i64, (bool, bool, bool))> { // 原子更新指定族开关与备份数（剔除 allowDirs）
    let (mut keep, (mut word, mut ppt, mut excel)) = read_office_config_in(writable, bundled);
    if let Some(val) = backup_keep { keep = val.clamp(1, 30); }
    if let Some(flag) = auto_screenshot {
        match family {
            "ppt" => ppt = flag,
            "excel" => excel = flag,
            _ => word = flag,
        }
    }
    let next = json!({"backup": {"keep": keep}, "auto_screenshot": {"word": word, "ppt": ppt, "excel": excel}});
    config::atomic_write_json(writable, &next)?;
    Ok((keep, (word, ppt, excel)))
}
fn cached_auto_flags() -> (bool, bool, bool) { // 读取三族自动截图开关缓存
    let mut guard = auto_cache().lock().unwrap_or_else(|err| err.into_inner());
    if let Some(flags) = *guard { return flags; }
    let (_, flags) = read_office_config_in(&config::WRITABLE_OFFICE_OPTION_FILE, &config::BUNDLED_OFFICE_OPTION_FILE);
    *guard = Some(flags);
    flags
}
pub fn auto_screenshot_enabled(family: &str) -> bool { // 查询指定族自动截图是否启用
    let (word, ppt, excel) = cached_auto_flags();
    match family { "ppt" => ppt, "excel" => excel, _ => word }
}
pub fn backup_keep() -> i64 { // 读取备份保留份数
    read_office_config_in(&config::WRITABLE_OFFICE_OPTION_FILE, &config::BUNDLED_OFFICE_OPTION_FILE).0
}
pub fn save_office_config(family: &str, backup_keep: Option<i64>, auto_screenshot: Option<bool>) -> anyhow::Result<(i64, bool)> { // 持久化 Office 配置并同步三族缓存
    let (keep, flags) = write_office_config_in(&config::WRITABLE_OFFICE_OPTION_FILE, &config::BUNDLED_OFFICE_OPTION_FILE, family, backup_keep, auto_screenshot)?;
    let mut guard = auto_cache().lock().unwrap_or_else(|err| err.into_inner());
    *guard = Some(flags);
    let family_flag = match family { "ppt" => flags.1, "excel" => flags.2, _ => flags.0 };
    Ok((keep, family_flag))
}
pub fn screenshot_dir() -> PathBuf { // Office 自动截图目录（static/data/office_screen）
    let path = config::writable_static_dir().join("data").join("office_screen");
    let _ = std::fs::create_dir_all(&path);
    path
}
pub fn clear_screenshots() -> u64 { // 清空 Office 自动截图目录
    sidecar_pipe::clear_screenshots_in(&screenshot_dir())
}
pub fn exe_path() -> PathBuf { config::app_dir().join("office_option").join("office_option.exe") } // 打包态可执行路径
fn dev_exe() -> PathBuf { config::app_dir().join("ai_office_option").join("target").join("release").join("office_option.exe") } // 开发态产物
fn seed_from_static() -> Option<PathBuf> { // 从随包目录播种
    let source = config::static_dir().join("office_option").join("office_option.exe"); // 随包副本
    if !source.is_file() { return None; } // 不存在
    let target = exe_path(); // 目标
    if let Some(parent) = target.parent() { let _ = std::fs::create_dir_all(parent); } // 建目录
    match std::fs::copy(&source, &target) { Ok(_) => { logger::info("office_option", &format!("已播种 sidecar: {}", target.display())); Some(target) } Err(err) => { logger::warn("office_option", &format!("播种 sidecar 失败: {err}")); None } } // 结果
}
fn resolve_exe() -> Option<PathBuf> { // 解析可执行文件
    if config::is_frozen() { if exe_path().is_file() { return Some(exe_path()); } return seed_from_static(); } // 打包态
    if dev_exe().is_file() { return Some(dev_exe()); } // 开发态
    let fallback = exe_path(); // 兜底
    if fallback.is_file() { Some(fallback) } else { None } // 返回
}
pub fn kill_sidecars() -> i32 { // 结束全部 office_option 进程树
    kill_sidecar_exe("office_option", "office_option.exe", true) // 统一解码 stdout/stderr 并清理进程树
}
fn start_locked() -> bool { // 启动 sidecar（调用方未持 SESSION 锁）
    { let state = enabled_guard(); if !(state.0 || state.1 || state.2) { return false; } } // 全部关闭不启动
    let Some(exe) = resolve_exe() else { logger::warn("office_option", "未找到 office_option.exe（先执行 ai_office_option/build.ps1）"); return false; }; // 无 exe
    kill_sidecars(); // 清理残留
    let mut child = match Command::new(&exe).args(["pipe", "--pipe", PIPE_NAME]).env("LOVEWORD_STATIC_DIR", config::writable_static_dir()).spawn() { Ok(child) => child, Err(err) => { logger::warn("office_option", &format!("启动 sidecar 失败: {err}")); return false; } }; // 拉起 并注入可写 static 根目录
    logger::info("office_option", &format!("sidecar 已拉起: {} pid={}", exe.display(), child.id())); // 记录
    let Some(pipe) = PipeChannel::connect("office_option", PIPE_NAME) else { let _ = child.kill(); return false; }; // 建立 PeekNamedPipe 通道
    let mut guard = session(); // 取锁
    *guard = Some(Session { child, pipe, next_id: 0, consecutive_timeouts: 0 }); // 保存
    true // 成功
}
fn timeout_error(timeout: Duration) -> String { format!("等待 Office 工具响应超时（{}s）", timeout.as_secs_f64()) } // 超时文案
fn read_response(session: &mut Session, id: u64, timeout: Duration) -> Result<(Value, Option<String>), String> { // 读取响应、重组分片并分离顶层 screenshot
    let deadline = Instant::now() + timeout; // 截止
    let mut parts: Vec<Option<String>> = Vec::new(); // 分片缓冲
    let mut shot: Option<String> = None; // 顶层截图路径
    loop { // 逐行
        let remaining = deadline.saturating_duration_since(Instant::now()); // 剩余
        if remaining.is_zero() { return Err(timeout_error(timeout)); } // 超时
        let text = match session.pipe.rx.recv_timeout(remaining) { Ok(text) => text, Err(RecvTimeoutError::Timeout) => return Err(timeout_error(timeout)), Err(RecvTimeoutError::Disconnected) => return Err("管道已断开".to_string()) }; // 取行
        if text.len() > MAX_FRAME { continue; } // 跳过超长
        let Ok(frame) = serde_json::from_str::<Value>(&text) else { continue; }; // 解析
        if frame.get("type").and_then(Value::as_str) != Some("res") { continue; } // 仅响应帧
        if frame.get("id").and_then(Value::as_u64) != Some(id) { continue; } // 匹配 id
        if let Some(found) = extract_frame_screenshot(&frame) { shot = Some(found); } // 提取顶层截图路径
        let chunks = frame.get("chunks").and_then(Value::as_u64).unwrap_or(1) as usize; // 分片数
        if chunks > 1 { // 分片
            let index = frame.get("chunk").and_then(Value::as_u64).unwrap_or(0) as usize; // 分片序号
            if parts.len() < chunks { parts.resize(chunks, None); } // 扩容
            if index < chunks { if let Some(data) = frame.get("data").and_then(Value::as_str) { parts[index] = Some(data.to_string()); } } // 落位
            if parts.iter().any(|part| part.is_none()) { continue; } // 未齐
            let joined: String = parts.iter().filter_map(|part| part.clone()).collect(); // 按序拼接
            let envelope: Value = serde_json::from_str(&joined).unwrap_or(Value::Null); // 完整信封
            if envelope.is_object() { return Ok((rebuild_envelope(&envelope, envelope.get("data").cloned().unwrap_or(Value::Null)), shot)); } // 重组
            return Ok((json!({"ok": frame.get("ok").and_then(Value::as_bool).unwrap_or(false), "data": Value::Null, "error": frame.get("error").cloned().unwrap_or(Value::Null)}), shot)); // 回退
        }
        return Ok((rebuild_envelope(&frame, frame.get("data").cloned().unwrap_or(Value::Null)), shot)); // 单帧
    }
}
fn rebuild_envelope(frame: &Value, data: Value) -> Value { // 重建信封：error 支持对象与字符串
    let ok = frame.get("ok").and_then(Value::as_bool).unwrap_or(false); // 成功判定
    let error = frame.get("error").cloned().unwrap_or(Value::Null); // error 原样
    json!({"ok": ok, "data": data, "error": error}) // 信封
}
fn reset_session() { // 丢弃会话并结束 sidecar
    let mut guard = session(); // 取锁
    *guard = None; // Drop 通道
    drop(guard); // 释放
    kill_sidecars(); // 结束进程
}
fn error_text(value: &Value) -> String { // 提取结构化或字符串错误文案
    match value.get("error") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Object(map)) => map.get("message").and_then(Value::as_str).unwrap_or("调用失败").to_string(),
        _ => "调用失败".to_string(),
    }
}
fn step_text(cmd: &str, args: &Value) -> String { // 生成 Office 操作可读中文步骤
    let family = if cmd.starts_with("ppt_") { "PPT" } else if cmd.starts_with("excel_") { "Excel" } else { "Word" }; // 族标签
    let op = args.get("action").or_else(|| args.get("target")).and_then(Value::as_str).unwrap_or_default(); // 子动作
    let file = args.get("path").and_then(Value::as_str).and_then(|p| p.rsplit(['\\', '/']).next()).unwrap_or_default(); // 文件名
    if !op.is_empty() && !file.is_empty() { format!("{family} {op} · {file}") } else if !op.is_empty() { format!("{family} {op}（{cmd}）") } else { format!("{family} {cmd}") }
}
pub fn steps(after: i64) -> (usize, Vec<Value>) { STEPS.list(after) } // 查询增量步骤
pub fn call(cmd: &str, args: &Value, timeout_secs: f64) -> Value { // 调用 Office 工具
    if !family_enabled(cmd) { return json!({"ok": false, "error": {"code": "GROUP_DISABLED", "message": "Office操作已关闭", "hint": "到设置启用对应工具", "recoverable": false}}); } // 组关闭硬拦截
    let started = Instant::now(); // 计时
    let mut guard = session(); // 全局锁
    if guard.as_mut().map(|item| item.child.try_wait().ok().flatten().is_some()).unwrap_or(true) { // 无会话或已退出
        drop(guard); // 释放锁
        if !start_locked() {
            STEPS.record(cmd, &step_text(cmd, args), args, false, &Value::Null, "Office 工具 sidecar 未就绪"); // 记录步骤
            return json!({"ok": false, "error": {"code": "SIDECAR_UNAVAILABLE", "message": "Office 工具 sidecar 未就绪", "hint": "检查 office_option.exe", "recoverable": true}});
        }
        guard = session(); // 重新取锁
    }
    let Some(active) = guard.as_mut() else { return json!({"ok": false, "error": "sidecar 未就绪"}); }; // 会话缺失
    while active.pipe.rx.try_recv().is_ok() {} // 清空迟到帧
    active.next_id += 1; // 分配 id
    let id = active.next_id; // 当前
    let mut payload = json!({"type": "req", "id": id, "cmd": cmd, "auto_screenshot": auto_screenshot_enabled(family_name(cmd))}); // 请求帧（附带当前族实时自动截图开关）
    if args.as_object().map(|map| !map.is_empty()).unwrap_or(false) { payload["args"] = args.clone(); } // 非空参数
    let line = format!("{payload}\n"); // 帧文本
    if let Err(err) = active.pipe.write_line(&line) {
        logger::warn("office_option", &format!("写管道失败: {err}"));
        drop(guard);
        reset_session();
        let msg = format!("写管道失败: {err}");
        STEPS.record(cmd, &step_text(cmd, args), args, false, &Value::Null, &msg); // 记录步骤
        return json!({"ok": false, "error": msg});
    }
    let timeout = Duration::from_secs_f64(timeout_secs.max(1.0)); // 超时
    match read_response(active, id, timeout) { // 读取
        Ok((value, shot)) => { // 成功
            active.consecutive_timeouts = 0; // 清零
            let clean = sidecar_pipe::strip_screenshot_fields(value); // 纵深剥离截图路径与图像字节，绝不返回大模型
            let ok = clean.get("ok").and_then(Value::as_bool).unwrap_or(false); // 判定
            let err = if ok { String::new() } else { error_text(&clean) }; // 错误文本
            logger::info("office_option", &format!("工具 {cmd} {}，耗时 {}ms", if ok { "完成" } else { "失败" }, started.elapsed().as_millis())); // 审计
            STEPS.record_with_screenshot(cmd, &step_text(cmd, args), args, ok, clean.get("data").unwrap_or(&Value::Null), &err, shot.as_deref()); // 记录步骤与可选截图路径
            clean // 返回干净信封
        }
        Err(err) => { // 失败
            let is_timeout = err.starts_with("等待 Office 工具响应超时"); // 是否超时
            STEPS.record(cmd, &step_text(cmd, args), args, false, &Value::Null, &err); // 记录步骤
            if is_timeout { // 超时不杀进程
                active.consecutive_timeouts += 1; // 计数
                while active.pipe.rx.try_recv().is_ok() {} // 丢迟到帧
                let count = active.consecutive_timeouts; // 当前
                logger::warn("office_option", &format!("工具 {cmd} 超时（连续 {count} 次）")); // 告警
                let result = json!({"ok": false, "error": {"code": "TIMEOUT", "message": err, "hint": "稍后重试", "recoverable": true}}); // 错误
                if count >= MAX_CONSECUTIVE_TIMEOUTS { drop(guard); reset_session(); } // 连续超时重启
                return result; // 返回
            }
            logger::warn("office_option", &format!("工具 {cmd} 调用失败: {err}")); // 告警
            drop(guard); // 释放
            reset_session(); // 重置
            json!({"ok": false, "error": err}) // 返回
        }
    }
}
pub fn snapshot() -> Value { // 状态快照
    let state = *enabled_guard(); // 开关快照
    let mut guard = session(); // 加锁
    let running = guard.as_mut().map(|item| item.child.try_wait().ok().flatten().is_none()).unwrap_or(false); // 运行
    let pid = guard.as_ref().map(|item| item.child.id()).unwrap_or(0); // pid
    json!({"running": running, "pid": pid, "pipe": running, "exe": exe_path().to_string_lossy(), "name": PIPE_NAME, "enabled": state.0 || state.1 || state.2, "word": state.0, "ppt": state.1, "excel": state.2}) // 快照
}
pub fn stop() { // 退出清理（try_lock 防持锁阻塞）
    if let Ok(mut guard) = SESSION.get_or_init(|| Mutex::new(None)).try_lock() {
        if let Some(active) = guard.as_mut() {
            active.next_id += 1; // 分配 id
            let id = active.next_id; // 当前
            let line = format!("{}\n", json!({"type": "req", "id": id, "cmd": "shutdown"})); // 关闭请求
            if active.pipe.write_line(&line).is_ok() { let _ = read_response(active, id, Duration::from_millis(600)); } // 短等待退出受控实例
        }
        *guard = None; // Drop 通道
    }
    let count = kill_sidecars(); // 结束进程（兜底）
    logger::info("office_option", &format!("退出清理 sidecar，命中 {count} 个进程")); // 记录
}
pub fn force_stop() { // 限时强退：不等待 RPC 响应，直接放行清理并强杀
    if let Ok(mut guard) = SESSION.get_or_init(|| Mutex::new(None)).try_lock() { *guard = None; }
    let count = kill_sidecars();
    logger::info("office_option", &format!("退出强清 sidecar，命中 {count} 个进程"));
}
#[cfg(test)] // 单元测试：组关闭硬拦截
mod tests { // 测试模块
    use super::*; // 引入被测函数
    #[test]
    fn group_disabled_blocks_tool_call() { // 关闭组后 call 直接返回 GROUP_DISABLED
        set_enabled("word", false); // 关闭
        set_enabled("ppt", false); // 关闭
        set_enabled("excel", false); // 关闭
        let result = call("word_document", &json!({"action": "status"}), 5.0); // 调用
        set_enabled("word", true); // 恢复
        set_enabled("ppt", true); // 恢复
        set_enabled("excel", true); // 恢复
        assert_eq!(result.get("ok").and_then(Value::as_bool), Some(false)); // 失败
        assert_eq!(result.get("error").and_then(|v| v.get("code")).and_then(Value::as_str), Some("GROUP_DISABLED")); // 错误码
    }
    #[test] // 独立三族自动截图开关持久化与 allowDirs 剥离
    fn independent_family_auto_screenshot_defaults_true_and_strips_allow_dirs() {
        let root = std::env::temp_dir().join(format!("lwa_office_cfg_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let writable = root.join("writable.json");
        let bundled = root.join("bundled.json");
        assert_eq!(read_office_config_in(&writable, &bundled), (5, (true, true, true)));
        std::fs::write(&bundled, r#"{"allowDirs":["C:/only"],"backup":{"keep":8},"auto_screenshot":{"word":false}}"#).unwrap();
        assert_eq!(read_office_config_in(&writable, &bundled), (8, (false, true, true)));
        let saved = write_office_config_in(&writable, &bundled, "ppt", Some(12), Some(false)).unwrap();
        assert_eq!(saved, (12, (false, false, true)));
        let disk = config::read_json(Some(&writable));
        assert!(disk.get("allowDirs").is_none());
        assert_eq!(disk["auto_screenshot"], json!({"word": false, "ppt": false, "excel": true}));
        assert_eq!(disk["backup"]["keep"], json!(12));
        let _ = std::fs::remove_dir_all(&root);
    }
}
