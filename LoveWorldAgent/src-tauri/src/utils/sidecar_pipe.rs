use base64::Engine; // Base64 编码
use serde_json::{json, Map, Value}; // sidecar 通用管道（PeekNamedPipe 非死锁读写）、控制台转码清理与操作步骤环形日志
use std::os::windows::process::CommandExt; // Windows 子进程创建标志
use std::path::{Path, PathBuf}; // 路径规范化与校验
use std::process::Command; // 外部命令调用
use std::sync::atomic::{AtomicBool, Ordering}; // 读线程停止标志
use std::sync::mpsc::{self, Receiver}; // 读线程消息通道
use std::sync::{Arc, Mutex}; // 共享标志与步骤锁
use std::thread::JoinHandle; // 读线程句柄
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH}; // 超时、退避与时间戳
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE}; // 句柄管理
use windows_sys::Win32::Globalization::{MultiByteToWideChar, CP_ACP, CP_OEMCP}; // Windows OEM/ANSI 代码页转码
use windows_sys::Win32::Storage::FileSystem::{ReadFile, WriteFile}; // 管道同步字节读写
use windows_sys::Win32::System::Pipes::{ConnectNamedPipe, CreateNamedPipeW, PeekNamedPipe}; // 命名管道创建与窥探
use crate::utils::logger; // 日志输出
const CREATE_NO_WINDOW: u32 = 0x0800_0000; // 不弹出控制台窗口
const PIPE_ACCESS_DUPLEX: u32 = 0x0000_0003; // 双向管道
const PIPE_TYPE_BYTE: u32 = 0x0000_0000; // 字节流模式
const PIPE_READMODE_BYTE: u32 = 0x0000_0000; // 字节流读取
const PIPE_WAIT: u32 = 0x0000_0000; // 阻塞连接模式
const PIPE_UNLIMITED_INSTANCES: u32 = 255; // 实例上限
const ERROR_MORE_DATA: i32 = 234; // Win32 缓冲续读码
const READ_CHUNK: usize = 65536; // 单次读块上限
const MAX_STEPS: usize = 200; // 步骤保存上限
pub struct PipeChannel { // 命名管道读写通道：PeekNamedPipe 读线程 + 循环 WriteFile，避免 Windows 同步句柄 ReadFile/WriteFile 内核死锁
    handle: isize, // 原生管道句柄
    pub rx: Receiver<String>, // 行接收端
    stop: Arc<AtomicBool>, // 读线程停止信号
    reader: Option<JoinHandle<()>>, // 读线程句柄
}
impl PipeChannel { // 管道通道方法
    pub fn connect(module: &str, pipe_name: &str) -> Option<Self> { // 创建管道实例、等待连接并启动 PeekNamedPipe 读线程
        let wide: Vec<u16> = format!("\\\\.\\pipe\\{pipe_name}").encode_utf16().chain(std::iter::once(0)).collect(); // 宽字符管道名
        let handle = unsafe { CreateNamedPipeW(wide.as_ptr(), PIPE_ACCESS_DUPLEX, PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT, PIPE_UNLIMITED_INSTANCES, 65536, 65536, 0, std::ptr::null()) }; // 创建字节流管道
        if handle == INVALID_HANDLE_VALUE { logger::warn(module, "创建命名管道失败"); return None; } // 创建失败
        let connected = unsafe { ConnectNamedPipe(handle, std::ptr::null_mut()) }; // 等待客户端接入
        if connected == 0 {
            let error = std::io::Error::last_os_error(); // 检查连接结果
            if error.raw_os_error() != Some(535) { logger::warn(module, &format!("等待管道连接失败: {error}")); unsafe { CloseHandle(handle) }; return None; } // 535 = 已连接
        }
        let raw_handle = handle as isize; // 转 isize 跨线程保存
        let stop = Arc::new(AtomicBool::new(false)); // 停止信号
        let stop_flag = stop.clone(); // 供读线程持有
        let (tx, rx) = mpsc::channel::<String>(); // 行通道
        let thread_name = format!("{module}-reader"); // 线程名
        let reader = match std::thread::Builder::new().name(thread_name).spawn(move || { // 窥探式读取循环，绝不长期阻塞在内核 ReadFile 上
            let mut buffer: Vec<u8> = Vec::new(); // 行缓冲区
            let mut last_active = Instant::now(); // 最近活动时间
            while !stop_flag.load(Ordering::Relaxed) { // 轮询直到停止或断线
                let mut available = 0u32; // 可读字节数
                let ok = unsafe { PeekNamedPipe(raw_handle as HANDLE, std::ptr::null_mut(), 0, std::ptr::null_mut(), &mut available, std::ptr::null_mut()) }; // 窥探可读字节
                if ok == 0 { break; } // 管道断开或错误
                if available == 0 { // 无数据则自适应退避
                    let idle_ms = last_active.elapsed().as_millis(); // 空闲时长
                    let sleep_ms = if idle_ms < 200 { 1 } else if idle_ms < 800 { 2 } else if idle_ms < 2500 { 5 } else { 15 }; // 1~15ms 退避
                    std::thread::sleep(Duration::from_millis(sleep_ms)); // 短暂休眠
                    continue; // 继续窥探
                }
                let mut chunk = vec![0u8; available.clamp(1, READ_CHUNK as u32) as usize]; // 按可读量分配缓冲
                let mut read = 0u32; // 实读字节
                let ok = unsafe { ReadFile(raw_handle as HANDLE, chunk.as_mut_ptr(), chunk.len() as u32, &mut read, std::ptr::null_mut()) }; // 仅在有数据时读取
                if ok == 0 { // 读失败检查续读
                    let err = std::io::Error::last_os_error().raw_os_error(); // 错误码
                    if err == Some(ERROR_MORE_DATA) && read > 0 { last_active = Instant::now(); buffer.extend_from_slice(&chunk[..read as usize]); continue; } // 缓冲不足续读
                    break; // 断线退出
                }
                if read == 0 { break; } // EOF 退出
                last_active = Instant::now(); // 刷新活跃时间
                buffer.extend_from_slice(&chunk[..read as usize]); // 追加数据
                while let Some(index) = buffer.iter().position(|byte| *byte == b'\n') { // 按换行拆帧
                    let line: Vec<u8> = buffer.drain(..=index).collect(); // 取出单行
                    let text = String::from_utf8_lossy(&line).trim().to_string(); // 转 UTF-8
                    if !text.is_empty() && tx.send(text).is_err() { return; } // 发送失败说明接收端已释放
                }
            }
        }) {
            Ok(handle) => handle, // 线程句柄
            Err(err) => { logger::warn(module, &format!("启动读线程失败: {err}")); unsafe { CloseHandle(handle) }; return None; } // 启动失败清理句柄
        };
        Some(Self { handle: raw_handle, rx, stop, reader: Some(reader) }) // 返回通道
    }
    pub fn write_line(&self, line: &str) -> Result<(), String> { // 写入完整行（循环 WriteFile 防部分写截断）
        if self.handle == 0 || self.handle == -1 { return Err("管道句柄无效".to_string()); } // 校验句柄
        let bytes = line.as_bytes(); // 字节切片
        let mut offset = 0usize; // 已写偏移
        while offset < bytes.len() { // 循环写完
            let mut written = 0u32; // 本次写入
            let ok = unsafe { WriteFile(self.handle as HANDLE, bytes[offset..].as_ptr(), (bytes.len() - offset) as u32, &mut written, std::ptr::null_mut()) }; // 写管道
            if ok == 0 || written == 0 { return Err(std::io::Error::last_os_error().to_string()); } // 写入失败
            offset += written as usize; // 推进偏移
        }
        Ok(()) // 成功
    }
}
impl Drop for PipeChannel { // 销毁前先停止读线程再关闭管道句柄
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed); // 通知读线程退出
        if let Some(handle) = self.reader.take() { let _ = handle.join(); } // 等待读线程安全退出（≤15ms）
        if self.handle != 0 && self.handle != -1 { unsafe { CloseHandle(self.handle as HANDLE) }; self.handle = 0; } // 释放句柄
    }
}
#[derive(Default)] // 步骤缓存状态
struct StepState { seq: u64, items: Vec<Value> } // 递增序号与环形列表
pub fn sanitize_payload(value: &Value, max_chars: usize) -> Value { // 过滤大体积 base64 并截断超长文本，供面板步骤展示
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => value.clone(),
        Value::String(text) => {
            if text.chars().count() <= max_chars { Value::String(text.clone()) } else { Value::String(format!("{}...(已截断，共 {} 字)", text.chars().take(max_chars).collect::<String>(), text.chars().count())) }
        }
        Value::Array(items) => Value::Array(items.iter().take(40).map(|item| sanitize_payload(item, 600)).collect()),
        Value::Object(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                if k == "base64" { out.insert(k.clone(), json!("<base64 omitted>")); } else { out.insert(k.clone(), sanitize_payload(v, 800)); }
            }
            Value::Object(out)
        }
    }
}
pub fn summarize_json(value: &Value, max_chars: usize) -> String { // 把返回值压成可读文本并限长
    if value.is_null() { return String::new(); }
    let clean = sanitize_payload(value, max_chars);
    let raw = match &clean {
        Value::String(text) => text.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
    };
    if raw.chars().count() <= max_chars { raw } else { format!("{}...(已截断)", raw.chars().take(max_chars).collect::<String>()) }
}
pub struct StepLog { // 线程安全的操作步骤记录器
    inner: Mutex<StepState>, // 内部互斥状态
}
impl StepLog { // 步骤记录器方法
    pub const fn new() -> Self { Self { inner: Mutex::new(StepState { seq: 0, items: Vec::new() }) } } // 常量构造
    pub fn record(&self, tool: &str, text: &str, args: &Value, ok: bool, result: &Value, error: &str) { // 追加一条含完整参数与返回值的操作步骤
        self.record_with_screenshot(tool, text, args, ok, result, error, None); // 兼容旧调用，不附带截图字段
    }
    pub fn record_with_screenshot(&self, tool: &str, text: &str, args: &Value, ok: bool, result: &Value, error: &str, screenshot: Option<&str>) { // 追加操作步骤（可选附带截图路径）
        let clean_args = sanitize_payload(args, 2000); // 保留完整入参摘要
        let result_text = if ok { summarize_json(result, 4000) } else { summarize_json(&json!({"error": error}), 4000) }; // 保留返回值或错误详情
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| (d.as_secs_f64() * 1000.0).round() / 1000.0).unwrap_or(0.0); // 秒级时间戳
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner()); // 加锁
        guard.seq += 1; // 递增序号
        let seq = guard.seq; // 当前序号
        let mut item = json!({"seq": seq, "ts": ts, "tool": tool, "text": text, "ok": ok, "error": error, "args": clean_args, "result": result_text}); // 构造步骤对象
        if let Some(shot) = screenshot.map(str::trim).filter(|s| !s.is_empty()) { item["screenshot"] = json!(shot); } // 仅非空时写入截图路径
        guard.items.push(item); // 写入步骤
        if guard.items.len() > MAX_STEPS { let drop_n = guard.items.len() - MAX_STEPS; guard.items.drain(..drop_n); } // 保留最近 200 条
    }
    pub fn list(&self, after: i64) -> (usize, Vec<Value>) { // 读取 seq > after 的增量步骤
        let guard = self.inner.lock().unwrap_or_else(|err| err.into_inner()); // 加锁
        let total = guard.items.len(); // 当前总数
        let items = guard.items.iter().filter(|item| item.get("seq").and_then(Value::as_i64).unwrap_or(0) > after).cloned().collect(); // 过滤增量
        (total, items) // 返回总数与增量
    }
}
pub fn truthy(value: &Value) -> bool { // JSON 值真值判定
    match value {
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().map(|item| item != 0.0).unwrap_or(true),
        Value::Null => false,
        Value::String(text) => !matches!(text.trim().to_lowercase().as_str(), "" | "false" | "0" | "no" | "off"),
        _ => true,
    }
}
pub fn extract_frame_screenshot(frame: &Value) -> Option<String> { // 从顶层响应帧提取非空截图路径（绝不写入 data）
    frame.get("screenshot").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}
pub fn strip_screenshot_fields(value: Value) -> Value { // 防御性递归剥离截图路径与图像字节字段，绝不透传给大模型
    match value {
        Value::Object(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                if matches!(k.as_str(), "screenshot" | "screenshot_path" | "screenshot_file_path" | "image_png" | "image_base64") { continue; }
                out.insert(k, strip_screenshot_fields(v));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(strip_screenshot_fields).collect()),
        other => other,
    }
}
pub fn resolve_screenshot_path_in(root: &Path, raw: &str) -> Option<PathBuf> { // 严格规范化并校验截图路径在 root 内且为文件
    let trimmed = raw.trim();
    if trimmed.is_empty() { return None; }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() { return None; }
    let canon_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let resolved = std::fs::canonicalize(&path).ok()?;
    if !resolved.starts_with(&canon_root) || !resolved.is_file() { return None; }
    Some(resolved)
}
pub fn clear_screenshots_in(dir: &Path) -> u64 { // 清理指定目录中的截图文件
    let Ok(entries) = std::fs::read_dir(dir) else { return 0; };
    let mut deleted = 0u64;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && std::fs::remove_file(&path).is_ok() { deleted += 1; }
    }
    deleted
}
pub fn read_screenshot_base64(root: &Path, raw: &str) -> Option<Value> { // 校验并读取指定截图目录内的图片为 Base64
    let target = resolve_screenshot_path_in(root, raw)?;
    let bytes = std::fs::read(&target).ok()?;
    let mime = match target.extension().map(|item| item.to_string_lossy().to_lowercase()).unwrap_or_default().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    };
    Some(json!({"path": target.to_string_lossy(), "mime": mime, "base64": base64::engine::general_purpose::STANDARD.encode(bytes)}))
}
fn decode_with_codepage(bytes: &[u8], codepage: u32) -> Option<String> { // 按指定 Windows 代码页转码为 UTF-8
    let Ok(len) = i32::try_from(bytes.len()) else { return None; }; // 长度越界保护
    if len == 0 { return Some(String::new()); } // 空切片直接返回空串
    let needed = unsafe { MultiByteToWideChar(codepage, 0, bytes.as_ptr(), len, std::ptr::null_mut(), 0) }; // 计算宽字符缓冲长度
    if needed <= 0 { return None; } // 转换失败返回空
    let mut wide = vec![0u16; needed as usize]; // 分配 UTF-16 缓冲
    let written = unsafe { MultiByteToWideChar(codepage, 0, bytes.as_ptr(), len, wide.as_mut_ptr(), needed) }; // 执行多字节到 UTF-16 转码
    if written <= 0 { return None; } // 写入失败返回空
    String::from_utf16(&wide[..written as usize]).ok() // UTF-16 转 Rust UTF-8
}
pub fn decode_console_bytes(bytes: &[u8]) -> String { // 解码 Windows 控制台字节流：优先 UTF-8，否则依次尝试 OEM/ANSI 代码页
    if bytes.is_empty() { return String::new(); } // 空输出返回空串
    if let Ok(text) = std::str::from_utf8(bytes) { return text.to_string(); } // 合法 UTF-8 直接透传
    decode_with_codepage(bytes, CP_OEMCP) // 先按控制台 OEM 代码页（如 中文版 CP936/GBK）解码
        .or_else(|| decode_with_codepage(bytes, CP_ACP)) // 再回退系统 ANSI 代码页
        .unwrap_or_else(|| String::from_utf8_lossy(bytes).to_string()) // 极端兜底
}
pub fn kill_sidecar_exe(module: &str, exe_name: &str, kill_tree: bool) -> i32 { // 统一无窗结束 sidecar 进程并按系统代码页解码日志
    let mut command = Command::new("taskkill"); // 调用系统 taskkill
    if kill_tree { command.args(["/F", "/T", "/IM", exe_name]); } else { command.args(["/F", "/IM", exe_name]); } // 按需决定是否杀整棵进程树
    command.creation_flags(CREATE_NO_WINDOW); // 隐藏控制台窗口避免闪屏
    match command.output() { // 执行并采集输出
        Ok(result) => { // 拿到进程输出
            let stdout = decode_console_bytes(&result.stdout); // 转码标准输出
            let stderr = decode_console_bytes(&result.stderr); // 转码标准错误
            let detail = if stderr.trim().is_empty() { String::new() } else { format!(" stderr={}", stderr.trim()) }; // 拼接非空错误流
            logger::info(module, &format!("清理 sidecar: {}{}", stdout.trim(), detail)); // 记录清晰可读中文日志
            if result.status.success() { 1 } else { 0 } // 成功命中返回 1，否则 0
        }
        Err(err) => { logger::warn(module, &format!("清理 sidecar 失败: {err}")); 0 } // 命令拉起失败告警
    }
}
#[cfg(test)] // 控制台编码解码单元测试
mod tests { // 测试 UTF-8 与 GBK 字节解码
    use super::*; // 引入被测函数
    #[test] // 验证 UTF-8 透传与 GBK 中文输出无乱码还原
    fn decodes_utf8_and_gbk_console_bytes() {
        assert_eq!(decode_console_bytes("错误: 没有找到进程".as_bytes()), "错误: 没有找到进程"); // UTF-8 直出
        let gbk_err: [u8; 4] = [0xB4, 0xED, 0xCE, 0xF3]; // GBK "错误" 字节序列
        let decoded = decode_console_bytes(&gbk_err); // 转码解码
        assert!(!decoded.contains('\u{FFFD}')); // 不得含替换菱形乱码符
    }
    #[test] // 验证旧版 record 保持无 screenshot 字段，record_with_screenshot 正确附带非空截图路径
    fn step_log_preserves_legacy_record_and_optional_screenshot() {
        let log = StepLog::new();
        log.record("office_read", "读取", &json!({"path": "a.docx"}), true, &json!({"ok": true}), "");
        log.record_with_screenshot("computer_click", "点击", &json!({"x": 10}), true, &json!({"path": "ax"}), "", None);
        log.record_with_screenshot("computer_click", "点击", &json!({"x": 20}), true, &json!({"path": "ax"}), "", Some("   "));
        log.record_with_screenshot("computer_click", "点击", &json!({"x": 30}), true, &json!({"path": "ax"}), "", Some("C:\\tmp\\shot.png"));
        let (total, items) = log.list(0);
        assert_eq!(total, 4);
        assert!(items[0].get("screenshot").is_none());
        assert!(items[1].get("screenshot").is_none());
        assert!(items[2].get("screenshot").is_none());
        assert_eq!(items[3].get("screenshot").and_then(Value::as_str), Some("C:\\tmp\\shot.png"));
    }
}
