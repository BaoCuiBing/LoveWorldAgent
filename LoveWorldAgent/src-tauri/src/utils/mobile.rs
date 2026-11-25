use serde_json::{json, Value}; // 移动端 sidecar 管道客户端（PeekNamedPipe 非死锁）与步骤日志
use std::path::PathBuf; // 路径
use std::process::{Child, Command}; // sidecar 进程
use std::sync::mpsc::RecvTimeoutError; // 超时错误
use std::sync::{Mutex, MutexGuard, OnceLock}; // 全局单例
use std::time::{Duration, Instant}; // 超时与耗时
use crate::utils::sidecar_pipe::{kill_sidecar_exe, PipeChannel, StepLog}; // 通用管道、进程清理与步骤记录
use crate::{config, logger}; // 配置与日志
pub const PIPE_NAME: &str = "loveworld_mobile"; // 管道名
const MAX_FRAME: usize = 8192; // 单帧读取上限
const MAX_CONSECUTIVE_TIMEOUTS: u32 = 2; // 连续超时达到该次数才重启 sidecar
struct Session { // 会话状态
    child: Child, // sidecar 进程
    pipe: PipeChannel, // 非死锁管道通道
    next_id: u64, // 请求 id 序列
    consecutive_timeouts: u32, // 连续超时计数
}
static SESSION: OnceLock<Mutex<Option<Session>>> = OnceLock::new(); // 全局会话
static STEPS: StepLog = StepLog::new(); // 操作步骤环形日志
fn session() -> MutexGuard<'static, Option<Session>> { SESSION.get_or_init(|| Mutex::new(None)).lock().unwrap_or_else(|err| err.into_inner()) } // 取全局锁
pub fn exe_path() -> PathBuf { config::app_dir().join("mobile").join("mobile_use.exe") } // 打包态可执行文件
fn dev_exe() -> PathBuf { config::app_dir().join("ai_mobile").join("target").join("release").join("mobile_use.exe") } // 开发态产物
fn seed_from_static() -> Option<PathBuf> { // 打包态从随包目录播种
    let source = config::static_dir().join("mobile").join("mobile_use.exe"); // 随包副本
    if !source.is_file() { return None; } // 不存在直接返回
    let target = exe_path(); // 目标路径
    if let Some(parent) = target.parent() { let _ = std::fs::create_dir_all(parent); } // 建目录
    match std::fs::copy(&source, &target) { Ok(_) => { logger::info("mobile", &format!("已播种 sidecar: {}", target.display())); Some(target) } Err(err) => { logger::warn("mobile", &format!("播种 sidecar 失败: {err}")); None } } // 复制结果
}
fn resolve_exe() -> Option<PathBuf> { // 解析可用可执行文件
    if config::is_frozen() { if exe_path().is_file() { return Some(exe_path()); } return seed_from_static(); } // 打包态优先可写目录
    if dev_exe().is_file() { return Some(dev_exe()); } // 开发态产物
    let fallback = exe_path(); // 兜底同打包路径
    if fallback.is_file() { Some(fallback) } else { None } // 返回结果
}
pub fn kill_sidecars() -> i32 { // 结束 mobile_use 进程（不杀进程树，避免连带杀掉共享 adb server）
    kill_sidecar_exe("mobile", "mobile_use.exe", false) // 统一解码输出并仅结束 mobile_use.exe
}
fn start_locked() -> bool { // 启动 sidecar 并建立管道（调用方未持 SESSION 锁）
    let Some(exe) = resolve_exe() else { logger::warn("mobile", "未找到 mobile_use.exe（开发态先执行 ai_mobile/build.ps1）"); return false; }; // 无可执行文件
    kill_sidecars(); // 清理残留
    let mut child = match Command::new(&exe).args(["pipe", "--pipe", PIPE_NAME]).spawn() { Ok(child) => child, Err(err) => { logger::warn("mobile", &format!("启动 sidecar 失败: {err}")); return false; } }; // 拉起 sidecar
    logger::info("mobile", &format!("sidecar 已拉起: {} pid={}", exe.display(), child.id())); // 记录
    let Some(pipe) = PipeChannel::connect("mobile", PIPE_NAME) else { let _ = child.kill(); return false; }; // 建立 PeekNamedPipe 通道
    let mut guard = session(); // 取锁
    *guard = Some(Session { child, pipe, next_id: 0, consecutive_timeouts: 0 }); // 保存会话
    true // 成功
}
fn timeout_error(timeout: Duration) -> String { format!("等待 sidecar 响应超时（{}s）", timeout.as_secs_f64()) } // 统一超时文案
fn read_response(session: &mut Session, id: u64, timeout: Duration) -> Result<Value, String> { // 读取与 id 匹配的响应并重组分片
    let deadline = Instant::now() + timeout; // 截止时刻
    let mut parts: Vec<String> = Vec::new(); // 分片缓冲
    loop { // 逐行读取
        let remaining = deadline.saturating_duration_since(Instant::now()); // 剩余时间
        if remaining.is_zero() { return Err(timeout_error(timeout)); } // 已超时
        let text = match session.pipe.rx.recv_timeout(remaining) { Ok(text) => text, Err(RecvTimeoutError::Timeout) => return Err(timeout_error(timeout)), Err(RecvTimeoutError::Disconnected) => return Err("管道已断开".to_string()) }; // 从读线程取行
        if text.len() > MAX_FRAME { continue; } // 跳过超长行
        let Ok(frame) = serde_json::from_str::<Value>(&text) else { continue; }; // 解析失败跳过
        if frame.get("type").and_then(Value::as_str) != Some("res") { continue; } // 只处理响应帧
        if frame.get("id").and_then(Value::as_u64) != Some(id) { continue; } // 非本次请求
        let chunks = frame.get("chunks").and_then(Value::as_u64).unwrap_or(1) as usize; // 分片总数
        if chunks > 1 { // 分片帧
            if let Some(data) = frame.get("data").and_then(Value::as_str) { parts.push(data.to_string()); } // 收集分片
            if parts.len() < chunks { continue; } // 未收齐继续读
            let joined: String = parts.concat(); // 拼接
            let data: Value = serde_json::from_str(&joined).unwrap_or(Value::Null); // 解析数据
            return Ok(json!({"ok": frame.get("ok").and_then(Value::as_bool).unwrap_or(true), "data": data, "error": frame.get("error").and_then(Value::as_str).unwrap_or_default()})); // 返回完整结果
        }
        return Ok(json!({"ok": frame.get("ok").and_then(Value::as_bool).unwrap_or(false), "data": frame.get("data").cloned().unwrap_or(Value::Null), "error": frame.get("error").and_then(Value::as_str).unwrap_or_default()})); // 单帧结果
    }
}
fn reset_session() { // 丢弃会话并结束 sidecar
    let mut guard = session(); // 取锁
    *guard = None; // Drop 通道
    drop(guard); // 先释放锁
    kill_sidecars(); // 结束进程
}
fn step_text(cmd: &str, args: &Value) -> String { // 把 mobile_* 工具调用转为可读中文步骤
    let app = args.get("app").and_then(Value::as_str).unwrap_or_default(); // 包名
    match cmd {
        "mobile_list_available_devices" => "列出已授权 Android 设备".to_string(),
        "mobile_list_apps" => "列出设备已安装应用".to_string(),
        "mobile_get_foreground_app" => "获取当前前台应用".to_string(),
        "mobile_launch_app" => if app.is_empty() { "启动手机应用".to_string() } else { format!("启动应用 {app}") },
        "mobile_terminate_app" => if app.is_empty() { "停止手机应用".to_string() } else { format!("停止应用 {app}") },
        "mobile_install_app" => "安装 APK".to_string(),
        "mobile_uninstall_app" => "卸载应用".to_string(),
        "mobile_get_screen_size" => "读取设备分辨率".to_string(),
        "mobile_click_on_screen_at_coordinates" => "点击设备屏幕".to_string(),
        "mobile_double_tap_on_screen" => "双击设备屏幕".to_string(),
        "mobile_long_press_on_screen_at_coordinates" => "长按设备屏幕".to_string(),
        "mobile_list_elements_on_screen" => "获取屏幕 UI 元素树".to_string(),
        "mobile_press_button" => format!("发送设备按键 {}", args.get("button").and_then(Value::as_str).unwrap_or_default()),
        "mobile_open_url" => "在设备打开网址".to_string(),
        "mobile_swipe_on_screen" => "滑动设备屏幕".to_string(),
        "mobile_type_keys" => "向设备输入文本".to_string(),
        "mobile_save_screenshot" | "mobile_take_screenshot" => "截取设备屏幕".to_string(),
        "mobile_set_orientation" => "设置屏幕方向".to_string(),
        "mobile_set_location" => "设置模拟定位".to_string(),
        "mobile_clipboard" => "读写设备剪贴板".to_string(),
        "mobile_get_device_logs" => "读取设备 logcat 日志".to_string(),
        "mobile_get_orientation" => "读取屏幕方向".to_string(),
        "mobile_start_screen_recording" => "开始设备录屏".to_string(),
        "mobile_stop_screen_recording" => "停止设备录屏".to_string(),
        "mobile_list_crashes" => "列出崩溃摘要".to_string(),
        "mobile_get_crash" => "读取崩溃详情".to_string(),
        "mobile_batch_commands" => "批量执行移动端命令".to_string(),
        other => other.to_string(),
    }
}
pub fn steps(after: i64) -> (usize, Vec<Value>) { STEPS.list(after) } // 查询增量步骤
pub fn call(cmd: &str, args: &Value, timeout_secs: f64) -> Value { // 调用 sidecar 工具
    let started = Instant::now(); // 记录耗时
    let mut guard = session(); // 全局锁
    if guard.as_mut().map(|item| item.child.try_wait().ok().flatten().is_some()).unwrap_or(true) { // 无会话或进程已退出
        drop(guard); // 先释放会话锁
        if !start_locked() {
            let err = "移动端 sidecar 未就绪";
            STEPS.record(cmd, &step_text(cmd, args), args, false, &Value::Null, err); // 记录失败步骤
            return json!({"ok": false, "error": err});
        }
        guard = session(); // 重新取锁
    }
    let Some(active) = guard.as_mut() else { return json!({"ok": false, "error": "移动端 sidecar 未就绪"}); }; // 会话缺失
    while active.pipe.rx.try_recv().is_ok() {} // 清空迟到帧
    active.next_id += 1; // 分配 id
    let id = active.next_id; // 当前 id
    let mut payload = json!({"type": "req", "id": id, "cmd": cmd}); // 请求帧
    if args.as_object().map(|map| !map.is_empty()).unwrap_or(false) { payload["args"] = args.clone(); } // 非空参数
    let line = format!("{payload}\n"); // 帧文本
    if let Err(err) = active.pipe.write_line(&line) {
        logger::warn("mobile", &format!("写管道失败: {err}"));
        drop(guard);
        reset_session();
        let msg = format!("写管道失败: {err}");
        STEPS.record(cmd, &step_text(cmd, args), args, false, &Value::Null, &msg); // 记录步骤
        return json!({"ok": false, "error": msg});
    }
    let timeout = Duration::from_secs_f64(timeout_secs.max(1.0)); // 超时
    match read_response(active, id, timeout) { // 读取响应
        Ok(value) => { // 成功
            active.consecutive_timeouts = 0; // 清零连续超时
            let ok = value.get("ok").and_then(Value::as_bool).unwrap_or(false); // 成功判定
            let err = if ok { "" } else { value.get("error").and_then(Value::as_str).unwrap_or("调用失败") }; // 错误文本
            logger::info("mobile", &format!("工具 {cmd} {}，耗时 {}ms", if ok { "完成" } else { "失败" }, started.elapsed().as_millis())); // 记录工具耗时
            STEPS.record(cmd, &step_text(cmd, args), args, ok, value.get("data").unwrap_or(&Value::Null), err); // 记录步骤
            value // 返回结果
        }
        Err(err) => { // 失败
            let is_timeout = err.starts_with("等待 sidecar 响应超时"); // 是否超时
            STEPS.record(cmd, &step_text(cmd, args), args, false, &Value::Null, &err); // 记录失败步骤
            if is_timeout { // 超时保留进程
                active.consecutive_timeouts += 1; // 计数
                while active.pipe.rx.try_recv().is_ok() {} // 丢弃迟到帧
                let count = active.consecutive_timeouts; // 当前计数
                logger::warn("mobile", &format!("工具 {cmd} 超时（连续 {count} 次）")); // 告警
                let result = json!({"ok": false, "error": err}); // 返回错误
                if count >= MAX_CONSECUTIVE_TIMEOUTS { drop(guard); logger::warn("mobile", "连续超时达到上限，重启 sidecar"); reset_session(); } // 连续超时才重启
                return result; // 返回超时错误
            }
            logger::warn("mobile", &format!("工具 {cmd} 调用失败: {err}")); // 记录失败
            drop(guard); // 释放锁
            reset_session(); // 重置会话
            json!({"ok": false, "error": err}) // 返回错误
        }
    }
}
pub fn snapshot() -> Value { // 返回 sidecar 状态
    let mut guard = session(); // 加锁
    let running = guard.as_mut().map(|item| item.child.try_wait().ok().flatten().is_none()).unwrap_or(false); // 运行状态
    let pid = guard.as_ref().map(|item| item.child.id()).unwrap_or(0); // 进程 id
    json!({"running": running, "pid": pid, "pipe": running, "exe": exe_path().to_string_lossy(), "name": PIPE_NAME}) // 状态快照
}
pub fn stop() { // 应用退出时结束 sidecar
    if let Ok(mut guard) = SESSION.get_or_init(|| Mutex::new(None)).try_lock() { *guard = None; } // 非阻塞释放会话
    let count = kill_sidecars(); // 结束进程
    logger::info("mobile", &format!("退出清理 sidecar，命中 {count} 个进程")); // 记录
}
