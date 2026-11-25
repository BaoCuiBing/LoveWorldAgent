use serde_json::{json, Value}; // 桌面自动化 sidecar 管道客户端与步骤日志
use std::path::{Path, PathBuf}; // 路径
use std::process::{Child, Command}; // sidecar 进程
use std::sync::mpsc::RecvTimeoutError; // 超时错误
use std::sync::{Mutex, MutexGuard, OnceLock}; // 全局单例
use std::time::{Duration, Instant}; // 超时与耗时
use crate::utils::sidecar_pipe::{self, extract_frame_screenshot, kill_sidecar_exe, PipeChannel, StepLog}; // 通用管道、进程清理、步骤记录与截图助手
pub use crate::utils::sidecar_pipe::{clear_screenshots_in, resolve_screenshot_path_in, strip_screenshot_fields}; // 复用共享截图安全助手
use crate::{config, logger}; // 配置与日志
pub const PIPE_NAME: &str = "loveworld_computer_use"; // 管道名
const MAX_FRAME: usize = 16384; // 单帧读取上限
const MAX_CONSECUTIVE_TIMEOUTS: u32 = 2; // 连续超时达到该次数才重启 sidecar
static AUTO_SCREENSHOT_CACHE: OnceLock<Mutex<Option<bool>>> = OnceLock::new(); // 自动截图开关缓存
fn auto_cache() -> &'static Mutex<Option<bool>> { AUTO_SCREENSHOT_CACHE.get_or_init(|| Mutex::new(None)) } // 缓存锁
pub fn read_auto_screenshot_from(writable: &Path, bundled: &Path) -> bool { // 从指定配置路径读取自动截图开关（默认 true）
    let path = if writable.exists() { Some(writable) } else if bundled.exists() { Some(bundled) } else { None };
    let data = config::read_json(path);
    data.get("auto_screenshot").map(sidecar_pipe::truthy).unwrap_or(true)
}
pub fn auto_screenshot_enabled() -> bool { // 读取自动截图开关（带内存缓存）
    let mut guard = auto_cache().lock().unwrap_or_else(|err| err.into_inner());
    if let Some(flag) = *guard { return flag; }
    let flag = read_auto_screenshot_from(&config::WRITABLE_COMPUTER_USE_FILE, &config::BUNDLED_COMPUTER_USE_FILE);
    *guard = Some(flag);
    flag
}
pub fn save_auto_screenshot(enabled: bool) -> anyhow::Result<bool> { // 持久化自动截图开关并刷新缓存
    let mut data = config::read_json(if config::WRITABLE_COMPUTER_USE_FILE.exists() { Some(config::WRITABLE_COMPUTER_USE_FILE.as_path()) } else { None });
    if let Some(map) = data.as_object_mut() {
        map.insert("auto_screenshot".to_string(), json!(enabled));
    } else {
        data = json!({"auto_screenshot": enabled});
    }
    config::atomic_write_json(&config::WRITABLE_COMPUTER_USE_FILE, &data)?;
    let mut guard = auto_cache().lock().unwrap_or_else(|err| err.into_inner());
    *guard = Some(enabled);
    Ok(enabled)
}
pub fn screenshot_dir() -> PathBuf { // 电脑控制截图目录（static/data/computer_screen）
    let path = config::writable_static_dir().join("data").join("computer_screen");
    let _ = std::fs::create_dir_all(&path);
    path
}
pub fn clear_screenshots() -> u64 { // 清空电脑控制截图目录
    clear_screenshots_in(&screenshot_dir())
}
struct Session { // 会话状态
    child: Child, // sidecar 进程
    pipe: PipeChannel, // 非死锁管道通道
    next_id: u64, // 请求 id 序列
    consecutive_timeouts: u32, // 连续超时计数
}
static SESSION: OnceLock<Mutex<Option<Session>>> = OnceLock::new(); // 全局会话
static RUNTIME_STATE: OnceLock<Mutex<(bool, u32)>> = OnceLock::new(); // 轻量运行态缓存（不跨越管道 I/O）
static STEPS: StepLog = StepLog::new(); // 操作步骤环形日志
fn session() -> MutexGuard<'static, Option<Session>> { SESSION.get_or_init(|| Mutex::new(None)).lock().unwrap_or_else(|err| err.into_inner()) } // 取全局锁
fn runtime_cache() -> &'static Mutex<(bool, u32)> { RUNTIME_STATE.get_or_init(|| Mutex::new((false, 0))) } // 运行态缓存锁
fn set_runtime(running: bool, pid: u32) { // 更新轻量运行态缓存
    let mut guard = runtime_cache().lock().unwrap_or_else(|err| err.into_inner());
    *guard = (running, pid);
}
fn get_runtime() -> (bool, u32) { // 读取轻量运行态缓存
    *runtime_cache().lock().unwrap_or_else(|err| err.into_inner())
}
pub fn exe_path() -> PathBuf { config::app_dir().join("computer_use").join("computer_use.exe") } // 打包态路径
fn dev_exe() -> PathBuf { config::app_dir().join("ai_computer_use").join("target").join("release").join("computer_use.exe") } // 开发态产物
fn copy_dir_missing(src: &std::path::Path, dst: &std::path::Path) { // 随包播种辅助：递归复制缺失目录文件
    let Ok(entries) = std::fs::read_dir(src) else { return; }; // 读取源目录
    let _ = std::fs::create_dir_all(dst); // 确保目标目录存在
    for entry in entries.flatten() { // 遍历条目
        let from = entry.path(); // 源路径
        let to = dst.join(entry.file_name()); // 目标路径
        if from.is_dir() { copy_dir_missing(&from, &to); } else if !to.exists() { let _ = std::fs::copy(&from, &to); } // 递归或复制
    }
}
fn seed_from_static() -> Option<PathBuf> { // 打包态从随包目录播种可执行文件及 omni_parser 资源
    let static_root = config::static_dir().join("computer_use"); // 随包根目录
    let source = static_root.join("computer_use.exe"); // 随包副本
    if !source.is_file() { return None; } // 不存在直接返回
    let target = exe_path(); // 目标路径
    if let Some(parent) = target.parent() { // 目标根存在
        let _ = std::fs::create_dir_all(parent); // 建目录
        for sub in ["models", "cuda_runtime"] { // 同步随包模型与 CUDA 运行时
            let sub_src = static_root.join(sub); // 源子目录
            if sub_src.is_dir() { copy_dir_missing(&sub_src, &parent.join(sub)); } // 补齐缺失资产
        }
    }
    match std::fs::copy(&source, &target) { Ok(_) => { logger::info("computer_use", &format!("已播种 sidecar: {}", target.display())); Some(target) } Err(err) => { logger::warn("computer_use", &format!("播种 sidecar 失败: {err}")); None } } // 复制结果
}
fn resolve_exe() -> Option<PathBuf> { // 解析可用可执行文件
    if config::is_frozen() { if exe_path().is_file() { return Some(exe_path()); } return seed_from_static(); } // 打包态优先可写目录
    if dev_exe().is_file() { return Some(dev_exe()); } // 开发态产物
    let fallback = exe_path(); // 兜底同打包路径
    if fallback.is_file() { Some(fallback) } else { None } // 返回结果
}
pub fn kill_sidecars() -> i32 { // 结束全部 sidecar 进程树
    kill_sidecar_exe("computer_use", "computer_use.exe", true) // 统一解码输出并清理进程树
}
fn start_locked() -> bool { // 启动 sidecar 并建立管道（调用方未持 SESSION 锁）
    let Some(exe) = resolve_exe() else { logger::warn("computer_use", "未找到 computer_use.exe（开发态先执行 ai_computer_use/build.ps1）"); set_runtime(false, 0); return false; }; // 无可执行文件
    kill_sidecars(); // 清理残留
    let mut child = match Command::new(&exe).args(["pipe", "--pipe", PIPE_NAME]).env("LOVEWORD_STATIC_DIR", config::writable_static_dir()).spawn() { Ok(child) => child, Err(err) => { logger::warn("computer_use", &format!("启动 sidecar 失败: {err}")); set_runtime(false, 0); return false; } }; // 拉起 sidecar 并注入可写 static 根目录
    let pid = child.id(); // 记录进程 id
    logger::info("computer_use", &format!("sidecar 已拉起: {} pid={pid}", exe.display())); // 记录
    let Some(pipe) = PipeChannel::connect("computer_use", PIPE_NAME) else { let _ = child.kill(); set_runtime(false, 0); return false; }; // 建立 PeekNamedPipe 通道
    let mut guard = session(); // 取锁
    *guard = Some(Session { child, pipe, next_id: 0, consecutive_timeouts: 0 }); // 保存会话
    drop(guard); // 释放会话锁
    set_runtime(true, pid); // 同步运行态缓存
    true // 成功
}
fn timeout_error(timeout: Duration) -> String { format!("等待 sidecar 响应超时（{}s）", timeout.as_secs_f64()) } // 统一超时文案
fn read_response(session: &mut Session, id: u64, timeout: Duration) -> Result<(Value, Option<String>), String> { // 读取与 id 匹配的响应并重组分片，分离顶层 screenshot
    let deadline = Instant::now() + timeout; // 截止时刻
    let mut parts: Vec<String> = Vec::new(); // 分片缓冲
    let mut shot: Option<String> = None; // 顶层截图路径
    loop { // 逐行读取
        let remaining = deadline.saturating_duration_since(Instant::now()); // 剩余时间
        if remaining.is_zero() { return Err(timeout_error(timeout)); } // 已超时
        let text = match session.pipe.rx.recv_timeout(remaining) { Ok(text) => text, Err(RecvTimeoutError::Timeout) => return Err(timeout_error(timeout)), Err(RecvTimeoutError::Disconnected) => return Err("管道已断开".to_string()) }; // 从读线程取行
        if text.len() > MAX_FRAME { continue; } // 跳过超长行
        let Ok(frame) = serde_json::from_str::<Value>(&text) else { continue; }; // 解析失败跳过
        if frame.get("type").and_then(Value::as_str) != Some("res") { continue; } // 只处理响应帧
        if frame.get("id").and_then(Value::as_u64) != Some(id) { continue; } // 非本次请求
        if let Some(found) = extract_frame_screenshot(&frame) { shot = Some(found); } // 提取顶层截图路径
        let chunks = frame.get("chunks").and_then(Value::as_u64).unwrap_or(1) as usize; // 分片总数
        if chunks > 1 { // 分片帧
            if let Some(data) = frame.get("data").and_then(Value::as_str) { parts.push(data.to_string()); } // 收集分片
            if parts.len() < chunks { continue; } // 未收齐继续读
            let joined: String = parts.concat(); // 拼接
            let data: Value = serde_json::from_str(&joined).unwrap_or(Value::Null); // 解析数据
            return Ok((json!({"ok": frame.get("ok").and_then(Value::as_bool).unwrap_or(true), "data": data, "error": frame.get("error").and_then(Value::as_str).unwrap_or_default()}), shot)); // 返回完整结果与分离截图
        }
        return Ok((json!({"ok": frame.get("ok").and_then(Value::as_bool).unwrap_or(false), "data": frame.get("data").cloned().unwrap_or(Value::Null), "error": frame.get("error").and_then(Value::as_str).unwrap_or_default()}), shot)); // 单帧结果与分离截图
    }
}
fn reset_session() { // 丢弃会话并结束 sidecar
    let mut guard = session(); // 取锁
    *guard = None; // Drop PipeChannel 停止读线程并关闭句柄
    drop(guard); // 先释放锁
    set_runtime(false, 0); // 刷新运行态缓存
    kill_sidecars(); // 结束进程树
}
fn step_text(cmd: &str, args: &Value) -> String { // 把 computer_* 工具调用转为可读中文步骤
    let app = args.get("app").or_else(|| args.get("name")).and_then(Value::as_str).unwrap_or_default(); // 可选应用名
    match cmd {
        "computer_list_apps" => "列出桌面与 UWP 应用".to_string(),
        "computer_list_windows" => "列出顶层窗口".to_string(),
        "computer_launch_app" => if app.is_empty() { "启动应用".to_string() } else { format!("启动应用 {app}") },
        "computer_kill_app" => "终止指定进程".to_string(),
        "computer_bring_to_front" => "置前并激活窗口".to_string(),
        "computer_set_window_frame" => "调整窗口位置与尺寸".to_string(),
        "computer_invoke_menu" => "调用原生窗口菜单".to_string(),
        "computer_get_window_state" => "感知窗口 UIA 与视觉状态".to_string(),
        "computer_get_desktop_state" => "感知全桌面视觉与窗口状态".to_string(),
        "computer_debug_window_info" => "诊断窗口焦点与属性".to_string(),
        "computer_get_accessibility_tree" => "获取桌面进程与窗口快照".to_string(),
        "computer_get_screen_size" => "读取屏幕分辨率与 DPI".to_string(),
        "computer_get_cursor_position" => "读取当前光标坐标".to_string(),
        "computer_click" => "点击目标元素或坐标".to_string(),
        "computer_double_click" => "双击目标元素或坐标".to_string(),
        "computer_right_click" => "右键点击目标".to_string(),
        "computer_drag" => "拖拽鼠标".to_string(),
        "computer_type_text" => "输入文本".to_string(),
        "computer_press_key" => format!("按下按键 {}", args.get("key").and_then(Value::as_str).unwrap_or_default()),
        "computer_hotkey" => "发送组合快捷键".to_string(),
        "computer_scroll" => format!("滚动 {}", args.get("direction").and_then(Value::as_str).unwrap_or_default()),
        "computer_set_value" => "设置控件文本值".to_string(),
        "computer_move_cursor" => "移动光标".to_string(),
        other => other.to_string(),
    }
}
pub fn steps(after: i64) -> (usize, Vec<Value>) { STEPS.list(after) } // 查询增量步骤
pub fn call(cmd: &str, args: &Value, timeout_secs: f64) -> Value { // 调用 sidecar 工具
    let started = Instant::now(); // 记录耗时
    let mut guard = session(); // 全局锁
    if guard.as_mut().map(|item| item.child.try_wait().ok().flatten().is_some()).unwrap_or(true) { // 无会话或进程已退出
        drop(guard); // 先释放会话锁
        set_runtime(false, 0); // 清理旧运行态缓存
        if !start_locked() {
            let err = "桌面自动化 sidecar 未就绪";
            STEPS.record(cmd, &step_text(cmd, args), args, false, &Value::Null, err); // 记录失败步骤
            return json!({"ok": false, "error": err});
        }
        guard = session(); // 重新取锁
    }
    let Some(active) = guard.as_mut() else { return json!({"ok": false, "error": "桌面自动化 sidecar 未就绪"}); }; // 会话缺失
    while active.pipe.rx.try_recv().is_ok() {} // 清空上次超时后迟到的帧
    active.next_id += 1; // 分配 id
    let id = active.next_id; // 当前 id
    let mut payload = json!({"type": "req", "id": id, "cmd": cmd, "auto_screenshot": auto_screenshot_enabled()}); // 请求帧（顶层附带实时自动截图开关）
    if args.as_object().map(|map| !map.is_empty()).unwrap_or(false) { payload["args"] = args.clone(); } // 非空参数
    let line = format!("{payload}\n"); // 帧文本
    if let Err(err) = active.pipe.write_line(&line) {
        logger::warn("computer_use", &format!("写管道失败: {err}"));
        drop(guard);
        reset_session();
        let msg = format!("写管道失败: {err}");
        STEPS.record(cmd, &step_text(cmd, args), args, false, &Value::Null, &msg); // 记录步骤
        return json!({"ok": false, "error": msg});
    }
    let timeout = Duration::from_secs_f64(timeout_secs.max(1.0)); // 超时
    match read_response(active, id, timeout) { // 读取响应
        Ok((value, shot)) => { // 成功
            active.consecutive_timeouts = 0; // 清零连续超时
            let clean = strip_screenshot_fields(value); // 防御性剥离任何截图路径或图像字段
            let ok = clean.get("ok").and_then(Value::as_bool).unwrap_or(false); // 成功判定
            let err = if ok { "" } else { clean.get("error").and_then(Value::as_str).unwrap_or("调用失败") }; // 错误文本
            logger::info("computer_use", &format!("工具 {cmd} {}，耗时 {}ms", if ok { "完成" } else { "失败" }, started.elapsed().as_millis())); // 记录工具耗时
            STEPS.record_with_screenshot(cmd, &step_text(cmd, args), args, ok, clean.get("data").unwrap_or(&Value::Null), err, shot.as_deref()); // 记录步骤与可选截图路径
            clean // 返回剥离后的干净结果
        }
        Err(err) => { // 失败
            let is_timeout = err.starts_with("等待 sidecar 响应超时"); // 是否超时
            STEPS.record(cmd, &step_text(cmd, args), args, false, &Value::Null, &err); // 记录失败步骤
            if is_timeout { // 超时保留进程与 element_id 缓存
                active.consecutive_timeouts += 1; // 计数
                while active.pipe.rx.try_recv().is_ok() {} // 丢弃迟到帧
                let count = active.consecutive_timeouts; // 当前计数
                logger::warn("computer_use", &format!("工具 {cmd} 超时（连续 {count} 次），保留 sidecar 缓存")); // 告警
                let result = json!({"ok": false, "error": err}); // 返回错误
                if count >= MAX_CONSECUTIVE_TIMEOUTS { drop(guard); logger::warn("computer_use", "连续超时达到上限，重启 sidecar"); reset_session(); } // 连续超时才重启
                return result; // 返回超时错误
            }
            logger::warn("computer_use", &format!("工具 {cmd} 调用失败: {err}")); // 记录失败
            drop(guard); // 释放锁
            reset_session(); // 管道断开或协议错误，重置会话
            json!({"ok": false, "error": err}) // 返回错误
        }
    }
}
pub fn snapshot() -> Value { // 返回 sidecar 状态（绝不阻塞在管道 I/O 的 SESSION 锁上）
    let try_guard = match SESSION.get_or_init(|| Mutex::new(None)).try_lock() {
        Ok(guard) => Some(guard),
        Err(std::sync::TryLockError::Poisoned(err)) => Some(err.into_inner()),
        Err(std::sync::TryLockError::WouldBlock) => None,
    };
    let (running, pid) = if let Some(mut guard) = try_guard {
        let running = guard.as_mut().map(|item| item.child.try_wait().ok().flatten().is_none()).unwrap_or(false); // 运行状态
        let pid = if running { guard.as_ref().map(|item| item.child.id()).unwrap_or(0) } else { 0 }; // 进程 id
        drop(guard); // 释放锁后再更新缓存
        set_runtime(running, pid); // 同步运行态缓存
        (running, pid)
    } else {
        get_runtime() // SESSION 正被长耗时 call() 持有，直接读取轻量运行态缓存
    };
    json!({"running": running, "pid": pid, "pipe": running, "exe": exe_path().to_string_lossy(), "name": PIPE_NAME}) // 状态快照
}
pub fn stop() { // 应用退出或手动停止时结束 sidecar
    if let Ok(mut guard) = SESSION.get_or_init(|| Mutex::new(None)).try_lock() { *guard = None; } // 非阻塞释放会话，避免工具持锁死锁 UI
    set_runtime(false, 0); // 立即刷新运行态缓存
    let count = kill_sidecars(); // 结束进程树（立即解阻塞任何在途管道）
    logger::info("computer_use", &format!("退出清理 sidecar，命中 {count} 个进程")); // 记录
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auto_screenshot_defaults_true_when_missing_or_invalid() {
        let root = std::env::temp_dir().join(format!("lwa_cu_cfg_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let writable = root.join("writable.json");
        let bundled = root.join("bundled.json");
        assert!(read_auto_screenshot_from(&writable, &bundled));
        std::fs::write(&bundled, "{}").unwrap();
        assert!(read_auto_screenshot_from(&writable, &bundled));
        std::fs::write(&bundled, "not-json").unwrap();
        assert!(read_auto_screenshot_from(&writable, &bundled));
        std::fs::write(&bundled, r#"{"auto_screenshot": false}"#).unwrap();
        assert!(!read_auto_screenshot_from(&writable, &bundled));
        std::fs::write(&writable, r#"{"auto_screenshot": true}"#).unwrap();
        assert!(read_auto_screenshot_from(&writable, &bundled));
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn screenshot_path_canonicalization_blocks_traversal() {
        let root = std::env::temp_dir().join(format!("lwa_cu_path_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let screen_dir = root.join("computer_screen");
        let nested_dir = screen_dir.join("sub");
        std::fs::create_dir_all(&nested_dir).unwrap();
        let valid_file = screen_dir.join("shot.png");
        let outside_file = root.join("outside.png");
        std::fs::write(&valid_file, b"png").unwrap();
        std::fs::write(&outside_file, b"out").unwrap();
        assert!(resolve_screenshot_path_in(&screen_dir, &valid_file.to_string_lossy()).is_some());
        assert!(resolve_screenshot_path_in(&screen_dir, "shot.png").is_none());
        assert!(resolve_screenshot_path_in(&screen_dir, &screen_dir.join("..").join("outside.png").to_string_lossy()).is_none());
        assert!(resolve_screenshot_path_in(&screen_dir, &outside_file.to_string_lossy()).is_none());
        assert!(resolve_screenshot_path_in(&screen_dir, &nested_dir.to_string_lossy()).is_none());
        assert!(resolve_screenshot_path_in(&screen_dir, &screen_dir.join("missing.png").to_string_lossy()).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn clear_screenshots_deletes_only_files_in_dir() {
        let root = std::env::temp_dir().join(format!("lwa_cu_clear_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let sub = root.join("keep_dir");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(root.join("a.png"), b"1").unwrap();
        std::fs::write(root.join("b.png"), b"2").unwrap();
        assert_eq!(clear_screenshots_in(&root), 2);
        assert!(sub.is_dir());
        assert!(!root.join("a.png").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn extract_frame_screenshot_reads_top_level_and_ignores_blank() {
        let frame = json!({"type": "res", "id": 1, "ok": true, "data": {"x": 1}, "screenshot": "  C:\\tmp\\a.png  "});
        assert_eq!(extract_frame_screenshot(&frame).as_deref(), Some("C:\\tmp\\a.png"));
        let empty = json!({"type": "res", "id": 2, "ok": true, "data": {}, "screenshot": "   "});
        assert!(extract_frame_screenshot(&empty).is_none());
    }
    #[test]
    fn snapshot_does_not_block_when_session_mutex_is_held() {
        let (locked_tx, locked_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let holder = std::thread::spawn(move || {
            let _guard = session();
            set_runtime(true, 4242);
            locked_tx.send(()).unwrap();
            let _ = release_rx.recv_timeout(Duration::from_secs(2));
            set_runtime(false, 0);
        });
        locked_rx.recv().unwrap();
        let started = Instant::now();
        let snap = snapshot();
        let elapsed = started.elapsed();
        let _ = release_tx.send(());
        holder.join().unwrap();
        assert!(elapsed < Duration::from_millis(100), "snapshot blocked for {elapsed:?}");
        assert_eq!(snap["running"], json!(true));
        assert_eq!(snap["pid"], json!(4242));
        assert_eq!(snap["pipe"], json!(true));
        assert_eq!(snap["name"], json!(PIPE_NAME));
        assert!(snap["exe"].is_string());
    }
}
