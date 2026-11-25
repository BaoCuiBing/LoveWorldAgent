use std::collections::HashMap; // 内置浏览器：CEF sidecar 启停、命名管道 v2 指令、操作步骤、截图与设置读写（对照 browser.py）
use std::os::windows::process::CommandExt; // Windows 进程创建标志
use std::path::{Path, PathBuf}; // 路径类型
use std::process::{Child, Command, Stdio}; // 子进程与管道
use std::sync::mpsc::{channel, Receiver, Sender}; // 同步应答与后台记步通道
use std::sync::Arc; // 管理器共享
use std::time::{Duration, Instant}; // 超时控制
use base64::Engine; // Base64 解码
use once_cell::sync::OnceCell; // 全局单例
use parking_lot::Mutex; // 互斥锁
use serde_json::{json, Value}; // JSON 构造
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE}; // 句柄关闭
use windows_sys::Win32::Storage::FileSystem::{ReadFile, WriteFile}; // 管道读写
use windows_sys::Win32::System::Pipes::{ConnectNamedPipe, CreateNamedPipeW, PeekNamedPipe}; // 命名管道 API
use crate::config; // 路径配置
use crate::utils::logger; // 日志输出
pub const PIPE_NAME: &str = "loveworld_browser_panel"; // 去掉 \\.\pipe\ 前缀，经 --pipe= 传给 sidecar
pub const READ_CHUNK: usize = 65536; // 单次 ReadFile 读块大小
pub const DEFAULT_TIMEOUT: f64 = 15.0; // 指令默认超时秒数
pub const MAX_STEPS: usize = 200; // 操作步骤保存上限
pub const SETTINGS_NAME: &str = "agent_browser.json"; // 设置文件名
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000; // 不弹出控制台窗口
const PIPE_ACCESS_DUPLEX: u32 = 0x0000_0003; // 双向管道
const PIPE_TYPE_BYTE: u32 = 0x0000_0000; // 字节流模式（配合换行分隔 JSON 帧，避免消息模式大帧被 ERROR_MORE_DATA 截断）
const PIPE_READMODE_BYTE: u32 = 0x0000_0000; // 字节流读取模式
const PIPE_WAIT: u32 = 0x0000_0000; // 阻塞模式
const ERROR_MORE_DATA: i32 = 234; // Win32 缓冲不足错误码（防御性续读）
static SETTINGS_CACHE: Mutex<Option<(bool, bool)>> = Mutex::new(None); // 内存缓存：(auto_screenshot, enabled)
pub fn bundled_file() -> PathBuf { // 随包设置文件路径
    config::LOVEWORD_DIR.join(SETTINGS_NAME) // 位于 loveword 目录
}
pub fn writable_file() -> PathBuf { // 可写设置文件路径
    config::WRITABLE_LOVEWORD_DIR.join(SETTINGS_NAME) // 位于可写 loveword 目录
}
pub fn settings_file() -> PathBuf { // 对外暴露的设置路径
    writable_file() // 返回可写路径
}
fn read_path() -> PathBuf { // 选取优先读取的设置路径
    if writable_file().exists() { writable_file() } else if bundled_file().exists() { bundled_file() } else { writable_file() } // 可写优先，回退随包
}
fn truthy(value: &Value) -> bool { // 把 JSON 值按真值语义转布尔
    match value { // 按类型分派
        Value::Bool(flag) => *flag, // 布尔直取
        Value::Number(number) => number.as_f64().map(|item| item != 0.0).unwrap_or(true), // 非零为真
        Value::Null => false, // null 为假
        Value::String(text) => !matches!(text.trim().to_lowercase().as_str(), "" | "false" | "0" | "no" | "off"), // 字符串假值列表
        _ => true, // 其他为真
    }
}
fn load_pair() -> (bool, bool) { // 从内存缓存或磁盘读取设置（首次惰性加载）
    let mut guard = SETTINGS_CACHE.lock();
    if let Some(pair) = *guard { return pair; }
    let extra = config::read_json(Some(&read_path()));
    let auto = extra.get("auto_screenshot").map(truthy).unwrap_or(true);
    let enabled = extra.get("enabled").map(truthy).unwrap_or(true);
    *guard = Some((auto, enabled));
    (auto, enabled)
}
pub fn auto_screenshot_enabled() -> bool { // 内存读取自动截图开关（无磁盘 IO）
    load_pair().0
}
pub fn load() -> Value { // 读取浏览器设置
    let (auto, enabled) = load_pair();
    json!({"auto_screenshot": auto, "enabled": enabled}) // 返回归一化设置
}
pub fn save(payload: &Value) -> Value { // 保存浏览器设置并刷新内存缓存
    let (mut auto, mut en) = load_pair();
    if let Some(value) = payload.get("auto_screenshot") { auto = truthy(value); }
    if let Some(value) = payload.get("enabled") { en = truthy(value); }
    *SETTINGS_CACHE.lock() = Some((auto, en));
    let current = json!({"auto_screenshot": auto, "enabled": en});
    if let Err(err) = config::atomic_write_json(&writable_file(), &current) { logger::warn("browser", &format!("写入浏览器设置失败: {err}")); } // 原子写入
    current // 返回设置
}
pub fn enabled() -> bool { // 内置浏览器是否启用（缺省 true）
    load_pair().1
}
pub fn write_sidecar_file() -> PathBuf { // 确保 sidecar 使用的设置文件存在
    let target = writable_file(); // 目标路径
    if !target.exists() { // 不存在则播种
        if let Err(err) = config::atomic_write_json(&target, &load()) { logger::warn("browser", &format!("写入浏览器设置文件失败: {err}")); } // 写入
    }
    target // 返回路径
}
pub fn bundled_dir() -> PathBuf { // 随包 sidecar 目录
    config::resource_dir().join("static").join("browser") // 位于资源目录
}
pub fn exe_path() -> PathBuf { // sidecar 可执行文件路径
    if config::is_frozen() { config::app_dir().join("browser").join("browser_panel.exe") } else { config::app_dir().join("ai_browser").join("out").join("Release").join("browser_panel.exe") } // 打包/开发两种路径
}
fn copy_tree(from: &Path, to: &Path) { // 递归复制目录
    for entry in walkdir::WalkDir::new(from).into_iter().flatten() { // 遍历源目录
        let relative = entry.path().strip_prefix(from).unwrap_or(entry.path()); // 相对路径
        let target = to.join(relative); // 目标路径
        if entry.file_type().is_dir() { let _ = std::fs::create_dir_all(&target); continue; } // 目录则创建
        if let Some(parent) = target.parent() { let _ = std::fs::create_dir_all(parent); } // 确保父目录
        let _ = std::fs::copy(entry.path(), &target); // 复制文件
    }
}
fn ensure_sidecar() -> PathBuf { // 确保 sidecar 已就绪，必要时从随包播种
    let target = exe_path(); // 目标路径
    if target.exists() { return target; } // 已存在直接返回
    let bundled = bundled_dir(); // 随包目录
    if !bundled.exists() { return target; } // 随包缺失返回原路径
    let parent = target.parent().map(Path::to_path_buf).unwrap_or_else(|| target.clone()); // 目标父目录
    copy_tree(&bundled, &parent); // 复制整个目录
    logger::info("browser", &format!("内置浏览器 sidecar 已播种: {}", parent.display())); // 记录
    target // 返回路径
}
pub fn cache_dir() -> PathBuf { // 浏览器与 Chromium 缓存统一目录（cache_path 与 root_cache_path 共用）
    let path = config::writable_static_dir().join("additional").join("cache"); // 固定到 static/additional/cache
    let _ = std::fs::create_dir_all(&path); // 确保存在
    path // 返回
}
pub fn screenshot_dir() -> PathBuf { // 截图目录
    let path = config::writable_static_dir().join("data").join("browser_screen"); // 路径
    let _ = std::fs::create_dir_all(&path); // 确保存在
    path // 返回
}
pub fn kill_sidecars() -> i32 { // 清理残留的浏览器进程
    let mut command = Command::new("taskkill"); // 用 taskkill
    command.args(["/F", "/T", "/IM", "browser_panel.exe"]).stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(CREATE_NO_WINDOW); // 强制终止整棵进程树
    match command.status() { // 执行
        Ok(status) => status.code().unwrap_or(-1), // 返回退出码
        Err(err) => { // 失败
            logger::warn("browser", &format!("清理残留 sidecar 失败: {err}")); // 记录告警
            -1 // 返回 -1
        }
    }
}
fn wide(text: &str) -> Vec<u16> { // 字符串转 UTF-16 并以 0 结尾（Windows API）
    text.encode_utf16().chain(std::iter::once(0)).collect() // 编码
}
fn now_seconds() -> f64 { // 当前 Unix 时间戳
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|value| value.as_secs_f64()).unwrap_or(0.0) // 系统时间转秒
}
fn millis() -> u128 { // 当前 Unix 毫秒
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|value| value.as_millis()).unwrap_or(0) // 系统时间转毫秒
}
fn truncate(text: String, limit: usize) -> String { // 按字符数截断
    text.chars().take(limit).collect() // 取前 limit 个字符
}
pub fn normalize_url(url: &str) -> String { // 规范化网址：空白回空串，缺协议补 https://（保留 data/blob/file/javascript/about）
    let text = url.trim();
    if text.is_empty() { return String::new(); }
    let lower = text.to_ascii_lowercase();
    if text.contains("://")
        || lower.starts_with("about:")
        || lower.starts_with("data:")
        || lower.starts_with("blob:")
        || lower.starts_with("file:")
        || lower.starts_with("javascript:")
    {
        text.to_string()
    } else {
        format!("https://{text}")
    }
}
fn write_pipe(handle: isize, line: &str) -> bool { // 向命名管道循环写入整行，避免大帧部分写截断
    if handle == 0 || handle == -1 { return false; } // 句柄无效
    let bytes = line.as_bytes();
    let mut offset = 0usize;
    while offset < bytes.len() {
        let mut written = 0u32;
        let ok = unsafe { WriteFile(handle as HANDLE, bytes[offset..].as_ptr(), (bytes.len() - offset) as u32, &mut written, std::ptr::null_mut()) };
        if ok == 0 || written == 0 { return false; }
        offset += written as usize;
    }
    true
}
struct StepJob { // 后台步骤与自动截图任务
    name: String,
    payload: Value,
    res: Value,
    take_shot: bool,
    session: u64,
}
struct Inner { // 浏览器管理器内部状态
    running: bool, // 进程是否运行
    starting: bool, // 是否正在启动
    visible: bool, // 窗口是否可见
    server_started: bool, // 管道服务是否已启动
    pid: u32, // 子进程 id
    handle: isize, // 管道句柄
    instance: isize, // 管道实例句柄
    proc: Option<Child>, // 子进程
    next_id: u64, // 下一个请求 id
    pending: HashMap<u64, Sender<Value>>, // 请求 id 到应答通道
    parts: HashMap<u64, (usize, HashMap<usize, String>, bool)>, // 分片消息重组 (chunks, map, is_json)
    title: String, // 页面标题
    address: String, // 页面地址
    loading: bool, // 是否加载中
    can_back: bool, // 可否后退
    can_forward: bool, // 可否前进
    rect: (i64, i64, i64, i64), // 窗口矩形
    tabs: Value, // 标签页列表
    active_tab: i64, // 当前标签
    dialog: Value, // 当前弹窗
    await_user: bool, // 是否等待用户操作
    steps: Vec<Value>, // 操作步骤
    step_seq: u64, // 步骤序号
    step_session: u64, // 会话代次（stop 时递增，丢弃旧会话在途后台记步）
    error: String, // 最近错误
}
impl Default for Inner { // 默认状态
    fn default() -> Self { // 构造默认
        Inner { running: false, starting: false, visible: false, server_started: false, pid: 0, handle: 0, instance: 0, proc: None, next_id: 0, pending: HashMap::new(), parts: HashMap::new(), title: String::new(), address: String::new(), loading: false, can_back: false, can_forward: false, rect: (0, 0, 0, 0), tabs: json!([]), active_tab: -1, dialog: Value::Null, await_user: false, steps: Vec::new(), step_seq: 0, step_session: 0, error: String::new() } // 全字段初始化
    }
}
pub struct BrowserManager { // 浏览器管理器
    inner: Mutex<Inner>, // 内部状态
    step_tx: Mutex<Option<Sender<StepJob>>>, // 后台记步/自动截图队列发送端
    write_lock: Mutex<()>, // 管道写入序列化锁，避免并发 write_pipe 交错字节流
}
static MANAGER: OnceCell<Arc<BrowserManager>> = OnceCell::new(); // 全局单例
static START_LOCK: Mutex<()> = Mutex::new(()); // 启动互斥锁
pub fn manager() -> Arc<BrowserManager> { // 获取全局管理器
    MANAGER.get_or_init(|| Arc::new(BrowserManager { inner: Mutex::new(Inner::default()), step_tx: Mutex::new(None), write_lock: Mutex::new(()) })).clone() // 首次访问初始化
}
fn step_text(name: &str, args: &Value) -> String { // 把工具调用转为可读步骤描述
    let text = |key: &str| args.get(key).map(|value| match value { Value::String(item) => item.clone(), other => other.to_string() }).unwrap_or_default(); // 取参数文本
    match name { // 按工具名分派
        "browser_navigate" => format!("导航到 {}", text("url")), // 导航
        "browser_navigate_back" => "后退".to_string(), // 后退
        "browser_tabs" => match text("action").as_str() { "list" => "列出标签页", "new" => "新建标签页", "close" => "关闭标签页", "select" => "选择标签页", _ => "操作标签页" }.to_string(), // 标签操作
        "browser_snapshot" | "browser_read_text" => "查看了网页内容".to_string(), // 查看内容
        "browser_wait_for" => if text("time").is_empty() { "等待页面变化".to_string() } else { format!("等待了 {} 秒", text("time")) }, // 等待
        "browser_console_messages" => "获取控制台消息".to_string(), // 控制台
        "browser_network_requests" => "获取网络请求".to_string(), // 网络请求
        "browser_take_screenshot" => "截取了屏幕截图".to_string(), // 截图
        "browser_click" => "点击了元素".to_string(), // 点击
        "browser_hover" => "悬停了元素".to_string(), // 悬停
        "browser_type" => "输入了文本".to_string(), // 输入
        "browser_select_option" => "选择了下拉项".to_string(), // 选择
        "browser_press_key" => format!("按了键 {}", text("key")), // 按键
        "browser_get_attribute" => format!("读取了元素属性 {}", text("attribute")), // 取属性
        "browser_evaluate" => "执行脚本".to_string(), // 执行脚本
        "browser_handle_dialog" => "处理了弹窗".to_string(), // 处理弹窗
        "browser_waiting_for_user_interaction" => format!("交还用户操作：{}", text("reason")), // 交还用户
        "ui_find" | "ui_find_clear" => "在页面中查找".to_string(), // 页面查找
        "ui_zoom" => format!("页面缩放 {}%", text("percent")), // 缩放
        "ui_clear_data" => "清空了浏览数据".to_string(), // 清空数据
        "ui_import_data" => "导入了浏览数据".to_string(), // 导入数据
        "ui_fullscreen" => "切换了全屏".to_string(), // 全屏
        other => other.to_string(), // 未知工具用原名
    }
}
impl BrowserManager { // 管理器方法
    fn is_running(&self) -> bool { // 判断 sidecar 进程是否运行
        let mut inner = self.inner.lock(); // 加锁
        match inner.proc.as_mut() { // 取子进程
            Some(child) => child.try_wait().map(|status| status.is_none()).unwrap_or(false), // 未退出即运行
            None => false, // 无进程
        }
    }
    pub fn snapshot(&self) -> Value { // 返回浏览器状态快照
        let running = self.is_running(); // 进程状态
        let inner = self.inner.lock(); // 加锁
        json!({"running": running, "pid": inner.pid, "pipe": inner.handle != 0, "title": inner.title, "address": inner.address, "loading": inner.loading, "canGoBack": inner.can_back, "canGoForward": inner.can_forward, "rect": [inner.rect.0, inner.rect.1, inner.rect.2, inner.rect.3], "tabs": inner.tabs, "activeTab": inner.active_tab, "dialog": inner.dialog, "awaitUser": inner.await_user, "visible": inner.visible, "starting": inner.starting, "error": inner.error}) // 投影字段
    }
    pub fn steps(&self) -> Vec<Value> { // 返回操作步骤
        self.inner.lock().steps.clone() // 克隆列表
    }
    fn add_step(&self, name: &str, args: &Value, res: &Value, shot: &str) { // 记录一步操作
        self.add_step_if_session(None, name, args, res, shot);
    }
    fn add_step_if_session(&self, expected_session: Option<u64>, name: &str, args: &Value, res: &Value, shot: &str) { // 按可选会话代次记录步骤（含完整参数与返回值）
        if !(name.starts_with("browser_") || name.starts_with("ui_") || name.starts_with("window_")) { return; } // 只记录相关工具
        let clean_args = crate::utils::sidecar_pipe::sanitize_payload(args, 2000); // 过滤大文本并保留完整参数
        let ok = res.get("ok").and_then(Value::as_bool).unwrap_or(false); // 成功标志
        let err = res.get("error").and_then(Value::as_str).unwrap_or_default(); // 错误文案
        let result_text = if ok { crate::utils::sidecar_pipe::summarize_json(res.get("data").unwrap_or(&Value::Null), 4000) } else { crate::utils::sidecar_pipe::summarize_json(&json!({"error": err}), 4000) }; // 返回值摘要
        let mut inner = self.inner.lock(); // 加锁
        if let Some(sess) = expected_session {
            if inner.step_session != sess { return; } // 已停止的旧会话在途任务直接丢弃
        }
        inner.step_seq += 1; // 序号递增
        let seq = inner.step_seq; // 当前序号
        let step = json!({"seq": seq, "ts": (now_seconds() * 1000.0).round() / 1000.0, "tool": name, "text": step_text(name, args), "ok": ok, "error": err, "screenshot": shot, "args": clean_args, "result": result_text}); // 构造步骤
        inner.steps.push(step); // 追加
        if inner.steps.len() > MAX_STEPS { // 超限裁剪
            let drop_count = inner.steps.len() - MAX_STEPS; // 超出数量
            inner.steps.drain(..drop_count); // 丢弃最旧
        }
    }
    fn send(&self, frame: Value) -> bool { // 向管道发送指令（等待管道就绪，写锁串行化防止并发交错）
        let line = format!("{}\n", frame); // 帧文本
        let label = frame.get("cmd").or_else(|| frame.get("type")).cloned().unwrap_or_default(); // 用于日志的标签
        let deadline = Instant::now() + Duration::from_secs(5); // 等待上限
        loop { // 重试等待
            let write_guard = self.write_lock.lock();
            let handle = self.inner.lock().handle; // 读句柄
            if handle != 0 { // 已连接
                if write_pipe(handle, &line) { return true; } // 写入成功
                logger::warn("browser", &format!("写浏览器管道失败: {label}")); // 写入失败
                return false; // 返回失败
            }
            drop(write_guard);
            if !self.inner.lock().running || Instant::now() >= deadline { // 未运行或超时
                logger::warn("browser", &format!("浏览器管道未就绪，丢弃指令: {label}")); // 记录
                return false; // 返回失败
            }
            std::thread::sleep(Duration::from_millis(50)); // 等待重试
        }
    }
    pub fn call(&self, cmd: &str, args: &Value, timeout: f64) -> Value { // 发起请求并等待应答
        if !self.is_running() { return json!({"ok": false, "error": "内置浏览器未启动"}); } // 未启动
        let (sender, receiver) = channel::<Value>(); // 应答通道
        let id = { // 登记请求
            let mut inner = self.inner.lock(); // 加锁
            if inner.handle == 0 { return json!({"ok": false, "error": "管道未连接，请稍候重试"}); } // 管道未连接
            inner.await_user = false; // 清除等待用户
            inner.next_id += 1; // 分配 id
            let id = inner.next_id; // 当前 id
            inner.pending.insert(id, sender); // 登记通道
            id // 返回 id
        };
        let mut payload = json!({"type": "req", "id": id, "cmd": cmd}); // 构造请求
        if args.as_object().map(|map| !map.is_empty()).unwrap_or(false) { payload["args"] = args.clone(); } // 非空参数才带上
        if !self.send(payload) { // 发送失败
            self.inner.lock().pending.remove(&id); // 清理登记
            return json!({"ok": false, "error": "管道写入失败，请稍候重试"}); // 返回失败
        }
        match receiver.recv_timeout(Duration::from_secs_f64(timeout.max(1.0))) { // 等待应答
            Ok(value) => { // 收到
                self.inner.lock().pending.remove(&id); // 清理登记
                value // 返回结果
            }
            Err(_) => { // 超时
                self.inner.lock().pending.remove(&id); // 清理登记
                json!({"ok": false, "error": format!("指令超时（{timeout}s）: {cmd}")}) // 返回超时错误
            }
        }
    }
    fn resolve(&self, frame: &Value) { // 处理应答帧，含分片重组（支持 format=json 自动还原对象）
        let Some(id) = frame.get("id").and_then(Value::as_u64) else { return; }; // 无 id 忽略
        let chunks = frame.get("chunks").and_then(Value::as_u64).unwrap_or(1) as usize; // 分片总数
        let mut inner = self.inner.lock(); // 加锁
        let result = if chunks > 1 { // 分片消息
            let chunk = frame.get("chunk").and_then(Value::as_u64).unwrap_or(0) as usize; // 当前分片号
            let text = frame.get("data").and_then(Value::as_str).unwrap_or_default().to_string(); // 分片数据
            let is_json = frame.get("format").and_then(Value::as_str) == Some("json");
            let entry = inner.parts.entry(id).or_insert_with(|| (chunks, HashMap::new(), false)); // 取或建重组槽
            entry.0 = chunks; // 更新分片数
            entry.2 |= is_json; // 记录是否 JSON 载荷
            entry.1.insert(chunk, text); // 保存分片
            if entry.1.len() < chunks { None } else { // 未收齐
                let (count, parts, as_json) = inner.parts.remove(&id).unwrap_or_default(); // 取出重组槽
                let text: String = (0..count).map(|index| parts.get(&index).cloned().unwrap_or_default()).collect(); // 按序拼接
                let data = if as_json { serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text)) } else { Value::String(text) };
                Some(json!({"ok": frame.get("ok").and_then(Value::as_bool).unwrap_or(false), "data": data, "error": frame.get("error").and_then(Value::as_str).unwrap_or_default()})) // 构造完整结果
            }
        } else { // 单帧消息
            Some(json!({"ok": frame.get("ok").and_then(Value::as_bool).unwrap_or(false), "data": frame.get("data").cloned().unwrap_or(Value::Null), "error": frame.get("error").and_then(Value::as_str).unwrap_or_default()})) // 直接构造结果
        };
        if let Some(value) = result { if let Some(sender) = inner.pending.remove(&id) { let _ = sender.send(value); } } // 投递应答
    }
    fn handle_ui(&self, data: &Value) { // 处理浏览器外壳动作
        let item = data.get("item").and_then(Value::as_str).unwrap_or_default().to_string(); // 动作名
        match item.as_str() { // 分派
            "screenshot" => { // 菜单截图
                let manager = manager(); // 管理器
                std::thread::Builder::new().name("browser-menu-shot".to_string()).spawn(move || { // 后台截图
                    let args = json!({"filename": format!("menu_{}.png", millis())}); // 文件名
                    let res = manager.take_screenshot(&args); // 截图
                    let path = if res.get("ok").and_then(Value::as_bool).unwrap_or(false) { res.get("data").and_then(|item| item.get("path")).and_then(Value::as_str).unwrap_or_default().to_string() } else { String::new() }; // 取路径
                    manager.add_step("browser_take_screenshot", &json!({}), &res, &path); // 记录步骤
                }).ok(); // 启动失败忽略
            }
            "openExternal" => { // 用系统浏览器打开链接
                let url = data.get("url").and_then(Value::as_str).unwrap_or_default().to_string(); // 目标 URL
                if url.is_empty() { return; } // 空 URL 跳过
                std::thread::Builder::new().name("browser-open-external".to_string()).spawn(move || { // 后台打开
                    match open::that(&url) { Ok(()) => logger::info("browser", &format!("已用系统默认浏览器打开: {}", truncate(url, 120))), Err(err) => logger::warn("browser", &format!("系统浏览器打开失败: {err}")) } // 记录结果
                }).ok(); // 启动失败忽略
            }
            other => logger::info("browser", &format!("浏览器外壳动作: {other}")), // 其他动作记日志
        }
    }
    fn apply_event(&self, frame: &Value) { // 处理浏览器事件，更新内部状态
        let event = frame.get("event").and_then(Value::as_str).unwrap_or_default().to_string(); // 事件名
        let data = frame.get("data").cloned().unwrap_or_else(|| json!({})); // 事件数据
        if event == "ui" { self.handle_ui(&data); return; } // 外壳动作单独处理
        logger::info("browser", &format!("浏览器事件 {event}: {}", truncate(data.to_string(), 160))); // 记录事件
        let mut inner = self.inner.lock(); // 加锁
        match event.as_str() { // 按事件分派
            "title" => inner.title = data.get("text").and_then(Value::as_str).unwrap_or_default().to_string(), // 标题变化
            "address" => inner.address = data.get("url").and_then(Value::as_str).unwrap_or_default().to_string(), // 地址变化
            "loading" => { // 加载状态
                inner.loading = data.get("value").and_then(Value::as_bool).unwrap_or(false); // 加载中
                inner.can_back = data.get("canGoBack").and_then(Value::as_bool).unwrap_or(false); // 可后退
                inner.can_forward = data.get("canGoForward").and_then(Value::as_bool).unwrap_or(false); // 可前进
            }
            "tabs" => { // 标签页变化并同步当前活动标签快照字段
                inner.tabs = data.get("tabs").cloned().unwrap_or_else(|| json!([])); // 标签列表
                inner.active_tab = data.get("active").and_then(Value::as_i64).unwrap_or(-1); // 当前标签
                if inner.active_tab >= 0 {
                    if let Some(tab) = data.get("tabs").and_then(Value::as_array).and_then(|arr| arr.get(inner.active_tab as usize)) {
                        if let Some(title) = tab.get("title").and_then(Value::as_str) { inner.title = title.to_string(); }
                        if let Some(url) = tab.get("url").and_then(Value::as_str) { inner.address = url.to_string(); }
                        if let Some(loading) = tab.get("loading").and_then(Value::as_bool) { inner.loading = loading; }
                        if let Some(can_back) = tab.get("canGoBack").and_then(Value::as_bool) { inner.can_back = can_back; }
                        if let Some(can_forward) = tab.get("canGoForward").and_then(Value::as_bool) { inner.can_forward = can_forward; }
                    }
                }
            }
            "dialog" => inner.dialog = data, // 弹窗信息
            "awaitUser" => { // 等待用户状态
                inner.await_user = data.get("awaitingUser").and_then(Value::as_bool).unwrap_or(false); // 等待标志
                inner.dialog = Value::Null; // 清除弹窗
            }
            "window" => inner.visible = data.get("visible").and_then(Value::as_bool).unwrap_or(false), // 窗口显隐
            _ => {} // 未知事件忽略
        }
    }
    fn handle_line(&self, line: &str) { // 处理管道一行消息
        let Ok(frame) = serde_json::from_str::<Value>(line) else { // 解析 JSON
            logger::info("browser", &format!("浏览器上报非 JSON: {}", truncate(line.to_string(), 120))); // 非 JSON 记录
            return; // 退出
        };
        match frame.get("type").and_then(Value::as_str).unwrap_or_default() { // 按类型分派
            "res" => self.resolve(&frame), // 应答
            "ev" => self.apply_event(&frame), // 事件
            kind => self.apply_event(&json!({"event": kind, "data": frame.clone()})), // 兼容旧格式
        }
    }
    fn serve(self: Arc<Self>) { // 命名管道服务循环（字节流模式 + ERROR_MORE_DATA 防御 + 自适应退避）
        let name = wide(&format!(r"\\.\pipe\{PIPE_NAME}")); // 管道名（宽字符）
        loop { // 外层重连循环
            let instance = unsafe { CreateNamedPipeW(name.as_ptr(), PIPE_ACCESS_DUPLEX, PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT, 1, (READ_CHUNK * 4) as u32, (READ_CHUNK * 4) as u32, 0, std::ptr::null()) }; // 创建字节流模式管道实例
            if instance == 0 || instance == -1 as HANDLE { // 创建失败
                logger::warn("browser", "创建浏览器管道失败，稍后重试"); // 记录
                std::thread::sleep(Duration::from_millis(500)); // 等待重试
                continue; // 下一轮
            }
            self.inner.lock().instance = instance as isize; // 保存实例句柄
            unsafe { ConnectNamedPipe(instance, std::ptr::null_mut()); } // 等待客户端连接
            self.inner.lock().handle = instance as isize; // 标记为已连接
            logger::info("browser", "浏览器管道已连接"); // 记录
            let mut buffer: Vec<u8> = Vec::new(); // 消息缓冲
            let mut last_active = Instant::now(); // 最近收发活动时间（驱动自适应轮询退避）
            loop { // 内层读取循环
                let handle = self.inner.lock().handle; // 取句柄
                if handle == 0 { break; } // 管道关闭
                let mut available = 0u32; // 可读字节数
                let ok = unsafe { PeekNamedPipe(handle as HANDLE, std::ptr::null_mut(), 0, std::ptr::null_mut(), &mut available, std::ptr::null_mut()) }; // 窥探可读字节
                if ok == 0 { break; } // 失败退出
                if available == 0 {
                    let idle_ms = last_active.elapsed().as_millis();
                    let sleep_ms = if idle_ms < 200 { 1 } else if idle_ms < 800 { 2 } else if idle_ms < 2500 { 5 } else { 15 };
                    std::thread::sleep(Duration::from_millis(sleep_ms));
                    continue;
                }
                let mut chunk = vec![0u8; available.clamp(1, READ_CHUNK as u32) as usize]; // 读取缓冲
                let mut read = 0u32; // 实际读取字节
                let ok = unsafe { ReadFile(handle as HANDLE, chunk.as_mut_ptr(), chunk.len() as u32, &mut read, std::ptr::null_mut()) }; // 读取
                if ok == 0 {
                    let err = std::io::Error::last_os_error().raw_os_error();
                    if err == Some(ERROR_MORE_DATA) && read > 0 {
                        last_active = Instant::now();
                        buffer.extend_from_slice(&chunk[..read as usize]);
                        continue;
                    }
                    break;
                }
                if read == 0 { break; }
                last_active = Instant::now();
                buffer.extend_from_slice(&chunk[..read as usize]); // 追加到缓冲
                while let Some(index) = buffer.iter().position(|byte| *byte == b'\n') { // 按行切分
                    let line: Vec<u8> = buffer.drain(..=index).collect(); // 取一行
                    let text = String::from_utf8_lossy(&line).trim().to_string(); // 解码
                    if !text.is_empty() { self.handle_line(&text); } // 处理
                }
            }
            { // 清理连接状态与未完成分片
                let mut inner = self.inner.lock(); // 加锁
                inner.handle = 0; // 清空句柄
                inner.instance = 0; // 清空实例
                inner.parts.clear(); // 清空断线残留分片
            }
            unsafe { CloseHandle(instance as HANDLE); } // 关闭管道实例
            std::thread::sleep(Duration::from_millis(500)); // 等待重连
        }
    }
    fn ensure_server(&self) { // 确保管道服务线程已启动
        { // 检查与标记临界区
            let mut inner = self.inner.lock(); // 加锁
            if inner.server_started { return; } // 已启动
            inner.server_started = true; // 标记启动
        }
        let manager = manager(); // 管理器
        std::thread::Builder::new().name("browser-pipe".to_string()).spawn(move || manager.serve()).ok(); // 后台运行服务
    }
    fn wait_pipe(&self, timeout: f64) -> bool { // 等待管道连接就绪
        let deadline = Instant::now() + Duration::from_secs_f64(timeout); // 截止时间
        while Instant::now() < deadline { // 轮询
            if self.inner.lock().handle != 0 { return true; } // 已连接
            std::thread::sleep(Duration::from_millis(100)); // 等待
        }
        self.inner.lock().handle != 0 // 最终状态
    }
    fn focus_locked(&self, url: &str, focus: bool) { // 复用会话时聚焦并可选导航
        if focus { // 需聚焦
            self.send(json!({"type": "show", "value": true})); // 显示窗口
            self.inner.lock().visible = true; // 更新可见状态
        }
        let target = normalize_url(url);
        if !target.is_empty() {
            let nav_res = self.call("browser_navigate", &json!({"url": target}), 10.0);
            if !nav_res.get("ok").and_then(Value::as_bool).unwrap_or(false) {
                logger::warn("browser", &format!("复用窗口导航失败: {}", nav_res.get("error").and_then(Value::as_str).unwrap_or_default()));
            }
        }
        if focus { // 需聚焦
            let res = self.call("window_focus", &json!({}), 3.0); // 请求聚焦
            if !res.get("ok").and_then(Value::as_bool).unwrap_or(false) { logger::info("browser", &format!("内置浏览器聚焦失败: {}", res.get("error").and_then(Value::as_str).unwrap_or_default())); } // 失败记日志
        }
    }
    fn spawn_locked(&self, rect: (f64, f64, f64, f64), url: &str, restarted: bool) -> Value { // 启动 sidecar 进程
        { // 重置状态
            let mut inner = self.inner.lock(); // 加锁
            inner.error.clear(); // 清空错误
            inner.running = false; // 重置运行标志
            inner.handle = 0; // 重置管道
            inner.proc = None; // 清空进程
            inner.visible = true; // 可见
        }
        kill_sidecars(); // 清理残留进程
        std::thread::sleep(Duration::from_millis(200)); // 等待清理
        let path = ensure_sidecar(); // 确保可执行文件就绪
        if !path.exists() { // 不存在
            let message = format!("sidecar 不存在: {}", path.display()); // 错误信息
            self.inner.lock().error = message.clone(); // 记录
            return json!({"ok": false, "error": message}); // 返回失败
        }
        let width = if rect.2.round() as i64 > 0 { rect.2.round() as i64 } else { 900 }; // 窗口宽
        let height = if rect.3.round() as i64 > 0 { rect.3.round() as i64 } else { 700 }; // 窗口高
        { // 更新运行标志
            let mut inner = self.inner.lock(); // 加锁
            inner.rect = (0, 0, width, height); // 记录矩形
            inner.running = true; // 标记运行
        }
        self.ensure_server(); // 启动管道服务
        let mut command = Command::new(&path); // 构造启动命令
        let cache = cache_dir(); // 统一缓存目录
        command.arg(format!("--rect=0,0,{},{}", width.max(480), height.max(360))).arg(format!("--pipe={PIPE_NAME}")).arg(format!("--cache={}", cache.display())).arg(format!("--root-cache={}", cache.display())).arg(format!("--settings={}", write_sidecar_file().display())); // 传入矩形/管道/缓存/根缓存/设置，GPU/Dawn 固定到 cache
        let norm_url = normalize_url(url);
        if !norm_url.is_empty() { command.arg(format!("--url={norm_url}")); } // 有初始 URL 则传入（由 sidecar 首标签加载，不再重复发 id=0 navigate）
        command.current_dir(path.parent().unwrap_or(Path::new("."))).stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(CREATE_NO_WINDOW); // 工作目录与静默启动
        match command.spawn() { // 启动进程
            Ok(child) => { // 成功
                let pid = child.id(); // 进程 id
                { // 保存子进程
                    let mut inner = self.inner.lock(); // 加锁
                    inner.proc = Some(child); // 保存
                    inner.pid = pid; // 记录 id
                }
                let connected = self.wait_pipe(5.0); // 等待管道连接
                logger::info("browser", &format!("浏览器窗口已启动: pid={pid} pipe={connected} restarted={restarted}")); // 记录
                json!({"ok": true, "running": true, "pid": pid, "pipe": connected, "restarted": restarted}) // 返回成功
            }
            Err(err) => { // 启动失败
                let message = format!("启动 sidecar 失败：{err}"); // 错误信息
                { // 更新状态
                    let mut inner = self.inner.lock(); // 加锁
                    inner.running = false; // 重置
                    inner.error = message.clone(); // 记录
                }
                json!({"ok": false, "error": message}) // 返回失败
            }
        }
    }
    pub fn wait_initial_ready(&self, has_url: bool) { // 新启动后同步等待首标签就绪（Queued in AgentApi::Execute until OnTabReady）与短暂首屏加载
        let _ = self.call("browser_tabs", &json!({"action": "list"}), 8.0);
        if has_url {
            let probe_deadline = Instant::now() + Duration::from_millis(300);
            while Instant::now() < probe_deadline {
                let (loading, has_title) = {
                    let inner = self.inner.lock();
                    (inner.loading, !inner.title.is_empty())
                };
                if loading || has_title { break; }
                std::thread::sleep(Duration::from_millis(25));
            }
            if self.inner.lock().loading {
                self.wait_loaded(3.0);
            }
        }
    }
    fn start_locked(&self, rect: (f64, f64, f64, f64), url: &str, focus: bool) -> Value { // 启动或复用浏览器
        if self.is_running() { // 已有进程
            if self.inner.lock().handle != 0 || self.wait_pipe(2.0) { // 管道就绪
                self.focus_locked(url, focus); // 聚焦并导航
                return json!({"ok": true, "running": true, "pid": self.inner.lock().pid, "pipe": true, "reused": true}); // 返回复用结果
            }
            logger::warn("browser", "浏览器进程存活但管道断线，清理后重启一次"); // 管道断线
            self.stop_locked(); // 停止
            return self.spawn_locked(rect, url, true); // 重启
        }
        self.spawn_locked(rect, url, false) // 首次启动
    }
    pub fn start(&self, rect: (f64, f64, f64, f64), url: &str, focus: bool) -> Value { // 同步启动入口
        let _guard = START_LOCK.lock(); // 持锁
        self.start_locked(rect, url, focus) // 委托
    }
    pub fn start_async(&self, rect: (f64, f64, f64, f64), url: &str) -> Value { // 异步启动入口
        { // 标记启动中
            let mut inner = self.inner.lock(); // 加锁
            if inner.starting { return json!({"ok": true, "accepted": true, "starting": true, "reused": true}); } // 已在启动
            inner.starting = true; // 标记
            inner.error.clear(); // 清空错误
        }
        let manager = manager(); // 管理器
        let target = url.to_string(); // 克隆 URL
        std::thread::Builder::new().name("browser-start".to_string()).spawn(move || { // 后台启动
            let result = manager.start(rect, &target, true); // 启动
            let mut inner = manager.inner.lock(); // 加锁
            if !result.get("ok").and_then(Value::as_bool).unwrap_or(false) { inner.error = result.get("error").and_then(Value::as_str).unwrap_or("启动失败").to_string(); } // 记录失败
            inner.starting = false; // 清除启动标志
        }).ok(); // 启动失败忽略
        json!({"ok": true, "accepted": true, "starting": true}) // 返回已受理
    }
    fn stop_locked(&self) -> Value { // 停止浏览器并清理状态
        { // 清理内部状态
            let mut inner = self.inner.lock(); // 加锁
            inner.running = false; // 重置运行
            inner.visible = false; // 隐藏
            inner.starting = false; // 清除启动
            inner.handle = 0; // 清空管道
            inner.pid = 0; // 清空 pid
            inner.proc = None; // 清空进程
            inner.steps.clear(); // 清空步骤
            inner.step_seq = 0; // 重置序号
            inner.step_session = inner.step_session.wrapping_add(1); // 递增会话代次，使在途后台记步失效
            inner.parts.clear(); // 清空分片
            for (_, sender) in inner.pending.drain() { let _ = sender.send(json!({"ok": false, "error": "浏览器已关闭"})); } // 唤醒所有等待请求
        }
        kill_sidecars(); // 终止进程
        logger::info("browser", "浏览器 sidecar 已停止"); // 记录
        json!({"ok": true, "running": false}) // 返回
    }
    pub fn stop(&self) -> Value { // 同步停止入口
        let _guard = START_LOCK.lock(); // 持锁
        self.stop_locked() // 委托
    }
    pub fn force_stop(&self) { // 退出专用非阻塞强清（防 START_LOCK 阻塞）
        if let Some(mut inner) = self.inner.try_lock() {
            inner.running = false;
            inner.visible = false;
            inner.starting = false;
            inner.handle = 0;
            inner.pid = 0;
            inner.proc = None;
            for (_, sender) in inner.pending.drain() { let _ = sender.send(json!({"ok": false, "error": "应用正在退出"})); }
        }
        kill_sidecars();
    }
    pub fn set_rect(&self, rect: (f64, f64, f64, f64)) -> Value { // 设置窗口矩形
        let width = if rect.2.round() as i64 > 0 { rect.2.round() as i64 } else { 900 }; // 宽
        let height = if rect.3.round() as i64 > 0 { rect.3.round() as i64 } else { 700 }; // 高
        json!({"ok": true, "rect": format!("0,0,{width},{height}"), "detached": true}) // 返回矩形
    }
    pub fn navigate(&self, url: &str) -> Value { // 导航到 URL
        let text = normalize_url(url); // 规范化并补协议
        if text.is_empty() { return json!({"ok": false, "error": "url 不能为空"}); } // 空 URL
        let res = self.call("browser_navigate", &json!({"url": text}), 10.0); // 调用工具（C++ 侧已做有界等待）
        json!({"ok": res.get("ok").and_then(Value::as_bool).unwrap_or(false), "url": text, "error": res.get("error").and_then(Value::as_str).unwrap_or_default()}) // 返回结果
    }
    pub fn show(&self, value: bool) -> Value { // 显隐浏览器窗口
        self.inner.lock().visible = value; // 更新状态
        self.send(json!({"type": "show", "value": value})); // 发送指令
        json!({"ok": true, "value": value}) // 返回
    }
    pub fn wait_loaded(&self, timeout: f64) -> bool { // 等待页面加载完成
        let deadline = Instant::now() + Duration::from_secs_f64(timeout); // 截止
        while Instant::now() < deadline { // 轮询
            if !self.inner.lock().loading { return true; } // 加载完成
            std::thread::sleep(Duration::from_millis(100)); // 等待
        }
        !self.inner.lock().loading // 最终状态
    }
    pub fn take_screenshot_with_timeout(&self, args: &Value, timeout: f64) -> Value { // 截图并落盘（支持自定义超时）
        let res = self.call("browser_take_screenshot", args, timeout); // 调用工具
        if !res.get("ok").and_then(Value::as_bool).unwrap_or(false) { return res; } // 失败直接返回
        let data = res.get("data").cloned().unwrap_or_else(|| json!({})); // 取数据
        let encoded = data.get("base64").and_then(Value::as_str).unwrap_or_default(); // 取 Base64
        if encoded.is_empty() { return json!({"ok": false, "error": "截图数据为空"}); } // 空数据
        let format = data.get("format").and_then(Value::as_str).unwrap_or("png"); // 图片格式
        let mut name = args.get("filename").and_then(Value::as_str).filter(|text| !text.is_empty()).map(str::to_string).unwrap_or_else(|| format!("browser_{}.png", chrono::Local::now().format("%Y%m%d_%H%M%S_%3f"))); // 文件名
        let lower = name.to_lowercase(); // 小写化
        if !(lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg")) { // 无合法后缀
            name.push('.'); // 补点
            name.push_str(format); // 补格式
        }
        let file = Path::new(&name).file_name().map(|item| item.to_string_lossy().to_string()).unwrap_or_else(|| "browser.png".to_string()); // 只取文件名
        let target = screenshot_dir().join(file); // 目标路径
        match base64::engine::general_purpose::STANDARD.decode(encoded) { // Base64 解码
            Ok(bytes) => match std::fs::write(&target, &bytes) { // 写文件
                Ok(()) => json!({"ok": true, "data": {"path": target.to_string_lossy(), "format": format, "bytes": bytes.len()}}), // 成功
                Err(err) => json!({"ok": false, "error": format!("截图保存失败：{err}")}), // 写失败
            },
            Err(err) => json!({"ok": false, "error": format!("截图保存失败：{err}")}), // 解码失败
        }
    }
    pub fn take_screenshot(&self, args: &Value) -> Value { // 截图并落盘（默认 25s 超时）
        self.take_screenshot_with_timeout(args, 25.0)
    }
    pub fn clear_screenshots(&self) -> Value { // 清空截图目录
        let dir = screenshot_dir(); // 目录
        let mut deleted = 0u64; // 删除计数
        if let Ok(entries) = std::fs::read_dir(&dir) { // 遍历
            for entry in entries.flatten() { // 逐项
                if entry.path().is_file() && std::fs::remove_file(entry.path()).is_ok() { deleted += 1; } // 删除文件
            }
        }
        json!({"ok": true, "data": {"deleted": deleted}}) // 返回删除数
    }
    fn should_auto_screenshot(name: &str, res_ok: bool) -> bool { // 判断是否触发后台自动截图
        res_ok
            && auto_screenshot_enabled()
            && !matches!(
                name,
                "browser_take_screenshot"
                    | "browser_console_messages"
                    | "browser_network_requests"
                    | "browser_get_attribute"
                    | "browser_state"
            )
            && !name.starts_with("window_")
            && !name.starts_with("ui_settings")
            && !name.starts_with("ui_import")
    }
    fn last_coalesced_shot_index(batch: &[StepJob]) -> Option<usize> { // 批量合并：只对最后一条需截图任务截一张图
        batch.iter().rposition(|job| job.take_shot)
    }
    fn step_worker_loop(rx: Receiver<StepJob>) { // 单线程串行记步 + 合并自动截图（FIFO 保证 BroweringPanel after=max(seq) 不丢缩略图）
        while let Ok(first) = rx.recv() {
            let mut batch = vec![first];
            while let Ok(next) = rx.try_recv() {
                batch.push(next);
            }
            let last_shot_idx = Self::last_coalesced_shot_index(&batch);
            let mgr = manager();
            for (idx, job) in batch.into_iter().enumerate() {
                let sess = job.session;
                let same_session = mgr.inner.lock().step_session == sess;
                let shot = if Some(idx) == last_shot_idx && same_session && mgr.is_running() {
                    let res = mgr.take_screenshot_with_timeout(
                        &json!({"type": "jpeg", "filename": format!("step_{}.jpg", millis())}),
                        3.0,
                    );
                    if res.get("ok").and_then(Value::as_bool).unwrap_or(false) {
                        res.get("data").and_then(|item| item.get("path")).and_then(Value::as_str).unwrap_or_default().to_string()
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                mgr.add_step_if_session(Some(sess), &job.name, &job.payload, &job.res, &shot);
            }
        }
    }
    fn ensure_step_worker(&self) -> Sender<StepJob> { // 惰性启动后台记步线程
        let mut guard = self.step_tx.lock();
        if let Some(tx) = guard.as_ref() { return tx.clone(); }
        let (tx, rx) = channel::<StepJob>();
        std::thread::Builder::new().name("browser-step-worker".to_string()).spawn(move || Self::step_worker_loop(rx)).ok();
        *guard = Some(tx.clone());
        tx
    }
    pub fn tool(&self, name: &str, args: &Value, timeout: f64) -> Value { // 统一工具入口：调用、后处理与异步非阻塞记步
        let payload = if args.is_object() { args.clone() } else { json!({}) }; // 参数归一
        if name == "browser_take_screenshot" { // 截图特殊处理
            let res = self.take_screenshot(&payload); // 截图
            let shot = if res.get("ok").and_then(Value::as_bool).unwrap_or(false) { res.get("data").and_then(|item| item.get("path")).and_then(Value::as_str).unwrap_or_default().to_string() } else { String::new() }; // 取路径
            self.add_step(name, &payload, &res, &shot); // 记步
            return res; // 返回
        }
        let session = self.inner.lock().step_session;
        let mut res = self.call(name, &payload, timeout); // 调用工具
        let res_ok = res.get("ok").and_then(Value::as_bool).unwrap_or(false);
        if name == "browser_navigate" && res_ok { // 导航已由 C++ 侧做有界等待，此处仅补齐状态快照
            let mut data = res.get("data").and_then(Value::as_object).cloned().unwrap_or_default(); // 取数据对象
            if !data.contains_key("state") {
                data.insert("state".to_string(), self.snapshot()); // 注入浏览器状态
            }
            res["data"] = Value::Object(data); // 写回
        }
        let take_shot = Self::should_auto_screenshot(name, res_ok);
        if auto_screenshot_enabled() {
            let tx = self.ensure_step_worker();
            if let Err(err) = tx.send(StepJob { name: name.to_string(), payload, res: res.clone(), take_shot, session }) {
                self.add_step_if_session(Some(session), name, &err.0.payload, &res, "");
            }
        } else {
            self.add_step_if_session(Some(session), name, &payload, &res, "");
        }
        res // 立即返回，0ms 阻塞
    }
    pub fn settings(&self) -> Value { // 返回设置并附加运行状态
        let mut data = load(); // 读取设置
        if let Some(map) = data.as_object_mut() { map.insert("running".to_string(), json!(self.is_running())); } // 注入运行状态
        data // 返回
    }
    pub fn save_settings(&self, payload: &Value) -> Value { // 保存设置
        let saved = save(payload); // 保存
        logger::info("browser", &format!("浏览器设置已保存: {}", truncate(saved.to_string(), 200))); // 记录
        saved // 返回
    }
    pub fn import_payload(&self, payload: &Value) -> Value { // 导入数据对象
        if !payload.as_object().map(|map| !map.is_empty()).unwrap_or(false) { return json!({"ok": false, "error": "数据为空"}); } // 空数据
        let res = self.call("ui_import_data", payload, 20.0); // 调用导入
        self.add_step("ui_import_data", &json!({}), &res, ""); // 记步
        res // 返回
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolve_reassembles_chunked_json_dict_and_text() {
        let mgr = BrowserManager { inner: Mutex::new(Inner::default()), step_tx: Mutex::new(None), write_lock: Mutex::new(()) };
        let (tx1, rx1) = channel::<Value>();
        let (tx2, rx2) = channel::<Value>();
        {
            let mut inner = mgr.inner.lock();
            inner.pending.insert(1, tx1);
            inner.pending.insert(2, tx2);
        }
        // Chunked JSON object (out of order chunks, format="json" on chunk 0)
        mgr.resolve(&json!({"type": "res", "id": 1, "ok": true, "chunks": 2, "chunk": 1, "data": ",\"format\":\"png\"}"}));
        assert!(rx1.try_recv().is_err());
        mgr.resolve(&json!({"type": "res", "id": 1, "ok": true, "chunks": 2, "chunk": 0, "format": "json", "data": "{\"base64\":\"YWJj\""}));
        let res1 = rx1.recv_timeout(Duration::from_secs(1)).expect("json reply");
        assert_eq!(res1["ok"], json!(true));
        assert_eq!(res1["data"]["base64"], json!("YWJj"));
        assert_eq!(res1["data"]["format"], json!("png"));

        // Chunked plain text (RespondText without format="json")
        mgr.resolve(&json!({"type": "res", "id": 2, "ok": true, "chunks": 2, "chunk": 0, "data": "hello "}));
        mgr.resolve(&json!({"type": "res", "id": 2, "ok": true, "chunks": 2, "chunk": 1, "data": "world"}));
        let res2 = rx2.recv_timeout(Duration::from_secs(1)).expect("text reply");
        assert_eq!(res2["data"], json!("hello world"));
    }

    #[test]
    fn apply_event_syncs_tabs_address_and_loading_state() {
        let mgr = BrowserManager { inner: Mutex::new(Inner::default()), step_tx: Mutex::new(None), write_lock: Mutex::new(()) };
        mgr.apply_event(&json!({
            "type": "ev",
            "event": "tabs",
            "data": {
                "n": 2,
                "active": 1,
                "tabs": [
                    {"index": 0, "title": "First", "url": "https://a.test", "loading": false, "canGoBack": false, "canGoForward": false, "active": false},
                    {"index": 1, "title": "Second", "url": "https://b.test", "loading": true, "canGoBack": true, "canGoForward": false, "active": true}
                ]
            }
        }));
        {
            let inner = mgr.inner.lock();
            assert_eq!(inner.active_tab, 1);
            assert_eq!(inner.title, "Second");
            assert_eq!(inner.address, "https://b.test");
            assert!(inner.loading);
            assert!(inner.can_back);
            assert!(!inner.can_forward);
            assert_eq!(inner.tabs.as_array().map(|a| a.len()), Some(2));
        }
        mgr.apply_event(&json!({"type": "ev", "event": "address", "data": {"url": "https://c.test"}}));
        mgr.apply_event(&json!({"type": "ev", "event": "loading", "data": {"value": false, "canGoBack": true, "canGoForward": true}}));
        let inner = mgr.inner.lock();
        assert_eq!(inner.address, "https://c.test");
        assert!(!inner.loading);
        assert!(inner.can_back);
        assert!(inner.can_forward);
    }

    #[test]
    fn normalize_url_handles_schemes_and_whitespace() {
        assert_eq!(normalize_url("  "), "");
        assert_eq!(normalize_url("github.com"), "https://github.com");
        assert_eq!(normalize_url("http://localhost:3000"), "http://localhost:3000");
        assert_eq!(normalize_url("https://example.com/path"), "https://example.com/path");
        assert_eq!(normalize_url("file:///C:/test.html"), "file:///C:/test.html");
        assert_eq!(normalize_url("file:/C:/test.html"), "file:/C:/test.html");
        assert_eq!(normalize_url("about:blank"), "about:blank");
        assert_eq!(normalize_url("ABOUT:blank"), "ABOUT:blank");
        assert_eq!(normalize_url("data:text/html,<h1>Hi</h1>"), "data:text/html,<h1>Hi</h1>");
        assert_eq!(normalize_url("DATA:text/plain,ok"), "DATA:text/plain,ok");
        assert_eq!(normalize_url("blob:https://example.com/id"), "blob:https://example.com/id");
        assert_eq!(normalize_url("javascript:void(0)"), "javascript:void(0)");
    }

    #[test]
    fn step_coalescing_picks_last_take_shot() {
        let batch = vec![
            StepJob { name: "browser_click".into(), payload: json!({}), res: json!({"ok": true}), take_shot: true, session: 0 },
            StepJob { name: "browser_type".into(), payload: json!({}), res: json!({"ok": true}), take_shot: true, session: 0 },
            StepJob { name: "browser_state".into(), payload: json!({}), res: json!({"ok": true}), take_shot: false, session: 0 },
        ];
        assert_eq!(BrowserManager::last_coalesced_shot_index(&batch), Some(1));
        assert_eq!(BrowserManager::last_coalesced_shot_index(&batch[2..]), None);
    }

    #[test]
    fn step_session_skips_stale_steps_after_stop() {
        let mgr = BrowserManager { inner: Mutex::new(Inner::default()), step_tx: Mutex::new(None), write_lock: Mutex::new(()) };
        let sess0 = mgr.inner.lock().step_session;
        mgr.add_step_if_session(Some(sess0), "browser_click", &json!({"ref": "e1"}), &json!({"ok": true}), "");
        assert_eq!(mgr.steps().len(), 1);
        {
            let mut inner = mgr.inner.lock();
            inner.steps.clear();
            inner.step_seq = 0;
            inner.step_session = inner.step_session.wrapping_add(1);
        }
        mgr.add_step_if_session(Some(sess0), "browser_type", &json!({"text": "stale"}), &json!({"ok": true}), "");
        assert!(mgr.steps().is_empty());
        let sess1 = mgr.inner.lock().step_session;
        mgr.add_step_if_session(Some(sess1), "browser_click", &json!({"ref": "e2"}), &json!({"ok": true}), "");
        assert_eq!(mgr.steps().len(), 1);
    }
}
