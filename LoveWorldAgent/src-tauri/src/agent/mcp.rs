use std::collections::HashMap; // MCP 配置与客户端：读写 .loveword/agent_mcp.json，并按 stdio/sse/http 三种传输接入服务
use serde_json::{json, Map, Value}; // JSON 构造与对象映射
use crate::config; // 配置文件路径与原子写
pub const TYPES: [&str; 3] = ["stdio", "sse", "http"]; // 支持的三种传输类型
const KNOWN_FIELDS: [&str; 9] = ["name", "type", "command", "args", "env", "url", "headers", "enabled", "disabled"]; // 归一化时保留的已知字段
fn is_cjk(ch: char) -> bool { // 判断字符是否为中日韩汉字
    ('\u{4e00}'..='\u{9fa5}').contains(&ch) // 汉字 Unicode 区间
}
pub fn valid_name(name: &str) -> bool { // 校验 MCP 服务名合法性（允许中文）
    let mut chars = name.chars(); // 字符迭代器
    let first_ok = chars.next().map(|ch| ch.is_ascii_alphanumeric() || is_cjk(ch)).unwrap_or(false); // 首字符须字母数字或汉字
    let count = name.chars().count(); // 名称长度
    first_ok && count <= 50 && chars.all(|ch| ch.is_ascii_alphanumeric() || is_cjk(ch) || matches!(ch, ' ' | '_' | '.' | '-')) // 长度与字符集校验
}
fn config_path() -> std::path::PathBuf { // 优先可写配置，回退随包配置
    let writable = &*config::WRITABLE_MCP_FILE; // 可写配置路径
    if writable.exists() { writable.clone() } else { config::BUNDLED_MCP_FILE.clone() } // 存在则用可写，否则随包
}
fn read_raw() -> Value { // 读取 MCP 配置原文
    let path = config_path(); // 解析配置路径
    if !path.exists() { return json!({}); } // 文件不存在返回空对象
    match std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok()) { // 读取并解析 JSON
        Some(value) if value.is_object() || value.is_array() => value, // 只接受对象或数组
        _ => json!({}), // 解析失败返回空对象
    }
}
pub fn load_servers() -> Vec<Value> { // 兼容多种格式加载服务列表
    let raw = read_raw(); // 读取原始配置
    let items: Vec<Value> = if let Some(array) = raw.as_array() { // 顶层即数组
        array.clone() // 直接返回
    } else if raw.get("servers").and_then(Value::as_array).map(|items| !items.is_empty()).unwrap_or(false) { // servers 数组非空
        raw.get("servers").and_then(Value::as_array).cloned().unwrap_or_default() // 取 servers
    } else if let Some(map) = raw.get("mcpServers").and_then(Value::as_object) { // Claude 风格 mcpServers 映射
        map.iter().map(|(name, item)| { // 逐个转为条目
            let mut entry = item.as_object().cloned().unwrap_or_default(); // 配置项对象
            entry.insert("name".to_string(), Value::String(name.clone())); // 注入名称
            Value::Object(entry) // 返回条目
        }).collect()
    } else { // 兜底取 servers
        raw.get("servers").and_then(Value::as_array).cloned().unwrap_or_default()
    };
    items.into_iter().filter(|item| item.is_object()).collect() // 只保留对象项
}
pub fn save_servers(items: &[Value]) { // 保存服务列表到可写配置
    let mut data = read_raw(); // 读取现有配置
    if !data.is_object() { data = json!({}); } // 非对象则重置
    if let Some(map) = data.as_object_mut() { map.remove("mcpServers"); map.insert("servers".to_string(), Value::Array(items.to_vec())); } // 统一写 servers，移除旧格式
    let path = &*config::WRITABLE_MCP_FILE; // 可写路径
    let _ = config::atomic_write_json(path, &data); // 原子写入
}
fn string_dict(value: Option<&Value>) -> Value { // 把键值对象归一化为字符串字典
    let mut map = Map::new(); // 结果映射
    if let Some(entries) = value.and_then(Value::as_object) { // 遍历对象
        for (key, item) in entries { // 逐键值
            if key.trim().is_empty() { continue; } // 跳过空键
            map.insert(key.clone(), Value::String(crate::state::json_str(Some(item)))); // 值转字符串
        }
    }
    Value::Object(map) // 返回字典
}
pub fn normalize(item: &Value) -> Value { // 归一化单个 MCP 服务配置
    let name = item.get("name").and_then(Value::as_str).unwrap_or_default().trim().to_string(); // 服务名
    let mut kind = item.get("type").and_then(Value::as_str).unwrap_or_default().trim().to_lowercase(); // 传输类型
    if !TYPES.contains(&kind.as_str()) { kind = if item.get("url").and_then(Value::as_str).map(|text| !text.trim().is_empty()).unwrap_or(false) { "http".to_string() } else { "stdio".to_string() }; } // 非法类型按 url 推断
    let args: Vec<Value> = match item.get("args") { // 启动参数，最多 50 项
        Some(Value::Array(items)) => items.iter().take(50).map(|value| Value::String(crate::state::json_str(Some(value)))).collect(), // 数组形式
        Some(Value::String(text)) => text.split_whitespace().take(50).map(|part| Value::String(part.to_string())).collect(), // 字符串形式按空白切
        _ => Vec::new(), // 其他为空
    };
    let enabled = item.get("enabled").and_then(Value::as_bool).unwrap_or(true) && !item.get("disabled").and_then(Value::as_bool).unwrap_or(false); // 启用需 enabled 且非 disabled
    let mut data = Map::new(); // 归一化结果
    data.insert("name".to_string(), Value::String(name)); // 服务名
    data.insert("type".to_string(), Value::String(kind)); // 传输类型
    data.insert("command".to_string(), Value::String(item.get("command").and_then(Value::as_str).unwrap_or_default().trim().to_string())); // 启动命令
    data.insert("args".to_string(), Value::Array(args)); // 参数列表
    data.insert("env".to_string(), string_dict(item.get("env"))); // 环境变量
    data.insert("url".to_string(), Value::String(item.get("url").and_then(Value::as_str).unwrap_or_default().trim().to_string())); // 服务 URL
    data.insert("headers".to_string(), string_dict(item.get("headers"))); // 请求头
    data.insert("enabled".to_string(), json!(enabled)); // 启用标志
    if let Some(entries) = item.as_object() { // 保留未知扩展字段
        for (key, value) in entries { // 遍历原始字段
            if KNOWN_FIELDS.contains(&key.as_str()) { continue; } // 已知字段跳过
            data.insert(key.clone(), value.clone()); // 未知字段保留
        }
    }
    Value::Object(data) // 返回归一化配置
}
pub fn validate(item: &Value) -> (Option<Value>, String) { // 校验服务配置并返回归一化结果或错误
    let data = normalize(item); // 先归一化
    let name = data.get("name").and_then(Value::as_str).unwrap_or_default(); // 服务名
    if !valid_name(name) { return (None, "名称需以字母、数字或中文开头，仅含中英文、数字、空格与 ._-（1-50 位）".to_string()); } // 名称非法
    let kind = data.get("type").and_then(Value::as_str).unwrap_or_default(); // 传输类型
    if kind == "stdio" { // stdio 需命令
        if data.get("command").and_then(Value::as_str).unwrap_or_default().is_empty() { return (None, "stdio 类型必须填写启动命令".to_string()); } // 命令为空
    } else { // sse/http 需 URL
        let url = data.get("url").and_then(Value::as_str).unwrap_or_default().to_lowercase(); // 服务 URL
        if !(url.starts_with("http://") || url.starts_with("https://")) { return (None, "sse/http 类型必须填写 http(s) 开头的 URL".to_string()); } // URL 非法
    }
    (Some(data), String::new()) // 校验通过
}
pub fn public(item: &Value) -> Value { // 对外展示：遮蔽 env/headers 敏感值
    let mut data = normalize(item); // 先归一化
    for key in ["env", "headers"] { // 遍历敏感字段
        if let Some(entries) = data.get(key).and_then(Value::as_object).cloned() { // 取字典
            let mut masked = Map::new(); // 遮蔽结果
            for name in entries.keys() { masked.insert(name.clone(), Value::String("****".to_string())); } // 全部替换为星号
            data[key] = Value::Object(masked); // 写回遮蔽值
        }
    }
    data // 返回脱敏配置
}
pub fn merge_item(existing: Option<&Value>, payload: &Value) -> Value { // 合并配置更新，保留未改的敏感值
    let mut merged = existing.and_then(Value::as_object).cloned().unwrap_or_default(); // 以现有配置为底
    if let Some(entries) = payload.as_object() { // 遍历提交字段
        for (key, value) in entries { merged.insert(key.clone(), value.clone()); } // 逐字段覆盖
    }
    for key in ["env", "headers"] { // 敏感字段特殊处理
        if let Some(values) = payload.get(key).and_then(Value::as_object) { // 提交的敏感字典
            let mut base = existing.and_then(|item| item.get(key)).and_then(Value::as_object).cloned().unwrap_or_default(); // 以原值为底
            for (name, value) in values { // 逐键值
                if crate::state::json_str(Some(value)) != "****" { base.insert(name.clone(), value.clone()); } // 非遮蔽占位才覆盖
            }
            merged.insert(key.to_string(), Value::Object(base)); // 写回合并结果
        }
    }
    normalize(&Value::Object(merged)) // 返回归一化配置
}
pub fn ensure_file() { // 确保可写 MCP 配置文件存在
    let writable = &*config::WRITABLE_MCP_FILE; // 可写路径
    if writable.exists() { return; } // 已存在直接返回
    let payload = if config::BUNDLED_MCP_FILE.exists() { read_raw() } else { json!({}) }; // 有随包配置则读，否则空
    let servers = payload.get("servers").and_then(Value::as_array).cloned().unwrap_or_default(); // 取服务列表
    save_servers(&servers); // 落盘
}
pub struct McpToolInfo { // 单个 MCP 工具信息
    pub name: String, // 工具名
    pub description: String, // 工具描述
    pub schema: Value, // 参数 Schema
}
struct StdioTransport { // stdio 传输：子进程 + 请求应答配对
    stdin: tokio::sync::Mutex<tokio::process::ChildStdin>, // 子进程标准输入
    child: tokio::sync::Mutex<tokio::process::Child>, // 子进程句柄
    pending: std::sync::Arc<parking_lot::Mutex<HashMap<u64, tokio::sync::oneshot::Sender<Value>>>>, // 请求 id 到应答通道
    next: std::sync::atomic::AtomicU64, // 请求 id 计数器
}
struct HttpTransport { // HTTP/SSE 传输
    client: reqwest::Client, // HTTP 客户端
    url: String, // 服务地址
    headers: Map<String, Value>, // 自定义请求头
    session: parking_lot::Mutex<String>, // MCP 会话 id
    next: std::sync::atomic::AtomicU64, // 请求 id 计数器
    sse_pending: std::sync::Arc<parking_lot::Mutex<HashMap<u64, tokio::sync::oneshot::Sender<Value>>>>, // SSE 应答通道
    endpoint: std::sync::Arc<parking_lot::Mutex<String>>, // SSE 事件端点
    endpoint_ready: std::sync::Arc<(parking_lot::Mutex<bool>, tokio::sync::Notify)>, // 端点就绪通知
}
enum Transport { // 传输方式枚举
    Stdio(StdioTransport), // stdio 子进程
    Http(HttpTransport), // HTTP 直连
    Sse(HttpTransport), // SSE 长连接
}
pub struct McpClient { // 单个 MCP 服务客户端
    pub name: String, // 服务名
    pub tools: Vec<McpToolInfo>, // 工具列表
    transport: Transport, // 底层传输
}
pub struct McpRuntime { // MCP 运行时快照：客户端 + 服务到工具名目录
    pub clients: Vec<std::sync::Arc<McpClient>>, // 已连接客户端
    pub catalog: std::collections::BTreeMap<String, Vec<String>>, // 服务名到工具名列表
}
pub struct McpManager { // 常驻管理器：状态、运行时、启动标志与重连互斥
    statuses: parking_lot::Mutex<std::collections::BTreeMap<String, Value>>, // 服务名 → 常驻状态（disabled/connecting/connected/error + tools/error/updated_at）
    runtime: parking_lot::Mutex<std::sync::Arc<McpRuntime>>, // 常驻客户端快照：聊天与状态查询共用，重连时整体替换
    started: std::sync::atomic::AtomicBool, // 首轮常驻连接是否已启动，未启动时聊天走 connect_all 兜底
    gate: tokio::sync::Mutex<()>, // 重连互斥：同一时刻只跑一组连接，避免重复拉起进程
}
static MANAGER: once_cell::sync::OnceCell<McpManager> = once_cell::sync::OnceCell::new(); // 全局单例管理器
fn now_seconds() -> f64 { // 当前 Unix 时间戳（秒）
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|item| item.as_secs_f64()).unwrap_or(0.0) // 系统时间转秒
}
pub fn manager() -> &'static McpManager { // 获取全局管理器单例
    MANAGER.get_or_init(|| McpManager { // 首次访问时初始化
        statuses: parking_lot::Mutex::new(std::collections::BTreeMap::new()), // 空状态表
        runtime: parking_lot::Mutex::new(std::sync::Arc::new(McpRuntime { clients: Vec::new(), catalog: std::collections::BTreeMap::new() })), // 空运行时
        started: std::sync::atomic::AtomicBool::new(false), // 未启动
        gate: tokio::sync::Mutex::new(()), // 空闲互斥
    })
}
impl McpManager { // 管理器操作
    pub fn statuses(&self) -> Value { // 返回所有服务状态
        let list: Map<String, Value> = self.statuses.lock().iter().map(|(name, item)| (name.clone(), item.clone())).collect(); // 复制状态表
        Value::Object(list) // 转为 JSON 对象
    }
    pub fn status_of(&self, name: &str) -> Option<Value> { // 查询单个服务状态
        self.statuses.lock().get(name).cloned() // 加锁读取
    }
    pub fn runtime(&self) -> std::sync::Arc<McpRuntime> { // 读取当前运行时快照
        self.runtime.lock().clone() // 克隆 Arc
    }
    pub fn started(&self) -> bool { // 是否已启动首轮连接
        self.started.load(std::sync::atomic::Ordering::SeqCst) // 原子读取
    }
    fn set_status(&self, name: &str, status: &str, tools: usize, error: &str) { // 更新服务状态
        self.statuses.lock().insert(name.to_string(), json!({"status": status, "tools": tools, "error": error, "updated_at": now_seconds()})); // 写入状态与时间戳
    }
    pub fn mark_error(&self, name: &str, error: &str) { // 标记服务为错误状态
        let tools = self.status_of(name).and_then(|item| item.get("tools").and_then(Value::as_u64)).unwrap_or(0) as usize; // 保留原工具数
        self.set_status(name, "error", tools, error); // 写入错误状态
        crate::utils::logger::warn("mcp", &format!("MCP 调用失败，标记 error: {name} ({error})")); // 记录告警
    }
    fn reset_statuses(&self, items: &[Value]) { // 按配置重置全部状态为连接中/禁用
        let mut list: std::collections::BTreeMap<String, Value> = std::collections::BTreeMap::new(); // 新状态表
        for raw in items { // 遍历服务配置
            let item = normalize(raw); // 归一化
            let name = item.get("name").and_then(Value::as_str).unwrap_or_default().to_string(); // 服务名
            if name.is_empty() { continue; } // 无名跳过
            let enabled = item.get("enabled").and_then(Value::as_bool).unwrap_or(true); // 启用状态
            let entry = if enabled { json!({"status": "connecting", "tools": 0, "error": "", "updated_at": now_seconds()}) } else { json!({"status": "disabled", "tools": 0, "error": "未启用", "updated_at": now_seconds()}) }; // 初始状态
            list.insert(name, entry); // 写入状态表
        }
        *self.statuses.lock() = list; // 整体替换
    }
    fn store_client(&self, client: std::sync::Arc<McpClient>) { // 写入或替换单个客户端
        let mut current = self.runtime.lock(); // 加锁运行时
        let name = client.name.clone(); // 服务名
        let mut clients: Vec<std::sync::Arc<McpClient>> = current.clients.iter().filter(|item| item.name != name).cloned().collect(); // 移除同名旧客户端
        let mut catalog = current.catalog.clone(); // 复制目录
        catalog.insert(name, client.tools.iter().map(|tool| tool.name.clone()).collect()); // 更新工具目录
        clients.push(client); // 追加新客户端
        *current = std::sync::Arc::new(McpRuntime { clients, catalog }); // 整体替换快照
    }
    fn drop_client(&self, name: &str) { // 移除单个客户端
        let mut current = self.runtime.lock(); // 加锁运行时
        let clients: Vec<std::sync::Arc<McpClient>> = current.clients.iter().filter(|item| item.name != name).cloned().collect(); // 过滤同名
        let mut catalog = current.catalog.clone(); // 复制目录
        catalog.remove(name); // 移除目录项
        *current = std::sync::Arc::new(McpRuntime { clients, catalog }); // 整体替换快照
    }
}
fn string_map(value: Option<&Value>) -> Vec<(String, String)> { // 对象转字符串键值对列表
    value.and_then(Value::as_object).map(|map| map.iter().map(|(key, item)| (key.clone(), crate::state::json_str(Some(item)))).collect()).unwrap_or_default() // 逐键值转换
}
fn rpc_error(payload: &Value) -> String { // 从 RPC 响应提取错误信息
    payload.get("error").map(|error| error.get("message").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| error.to_string())).unwrap_or_else(|| "MCP 调用失败".to_string()) // 优先 message 字段
}
async fn post_json(client: &reqwest::Client, url: &str, headers: &Map<String, Value>, payload: &Value, session: &str) -> Result<reqwest::Response, String> { // 发送 MCP JSON-RPC POST 请求
    let mut request = client.post(url).header("Content-Type", "application/json").header("Accept", "application/json, text/event-stream"); // 构造请求，声明两种响应格式
    for (key, item) in headers { request = request.header(key, crate::state::json_str(Some(item))); } // 注入自定义请求头
    if !session.is_empty() { request = request.header("Mcp-Session-Id", session); } // 有会话 id 则带上
    request.json(payload).send().await.map_err(|err| err.to_string()) // await 发送
}
fn parse_sse_text(body: &str) -> Vec<Value> { // 解析 SSE 文本为 JSON 消息列表
    body.split("\n\n").filter_map(|chunk| { // 按空行切事件块
        let data: Vec<String> = chunk.lines().filter_map(|line| line.strip_prefix("data:").map(|text| text.trim().to_string())).collect(); // 提取 data 行
        if data.is_empty() { return None; } // 无 data 跳过
        serde_json::from_str::<Value>(&data.join("\n")).ok() // 解析 JSON，失败跳过
    }).collect()
}
async fn response_payload(response: reqwest::Response) -> Result<Value, String> { // 从 HTTP 响应提取 RPC 结果（兼容 SSE）
    let content_type = response.headers().get("content-type").and_then(|value| value.to_str().ok()).unwrap_or("").to_lowercase(); // 响应内容类型
    let text = response.text().await.map_err(|err| err.to_string())?; // 读取响应正文
    if content_type.contains("text/event-stream") { // SSE 响应
        for item in parse_sse_text(&text) { if item.get("id").is_some() && (item.get("result").is_some() || item.get("error").is_some()) { return Ok(item); } } // 找带 id 的结果帧
        return Err("MCP 响应缺少结果".to_string()); // 无结果帧报错
    }
    serde_json::from_str::<Value>(&text).map_err(|err| err.to_string()) // 普通 JSON 响应
}
impl StdioTransport { // stdio 传输实现
    async fn request(&self, method: &str, params: Value) -> Result<Value, String> { // 发起请求并等待应答
        let id = self.next.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1; // 分配请求 id
        let (sender, receiver) = tokio::sync::oneshot::channel(); // 创建应答通道
        self.pending.lock().insert(id, sender); // 登记通道
        let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string(); // 构造 JSON-RPC 行
        { // 写入子进程的临界区
            use tokio::io::AsyncWriteExt; // 异步写 trait
            let mut stdin = self.stdin.lock().await; // 加锁标准输入
            stdin.write_all(format!("{line}\n").as_bytes()).await.map_err(|err| err.to_string())?; // 写入请求行
            stdin.flush().await.map_err(|err| err.to_string())?; // 刷送
        }
        match tokio::time::timeout(std::time::Duration::from_secs(30), receiver).await { // 等待应答，30 秒超时
            Ok(Ok(value)) => { if value.get("error").is_some() { Err(rpc_error(&value)) } else { Ok(value.get("result").cloned().unwrap_or(Value::Null)) } } // 分类处理应答
            _ => { self.pending.lock().remove(&id); Err(format!("MCP 请求超时: {method}")) } // 超时清理并返回错误
        }
    }
    async fn notify(&self, method: &str) { // 发送无 id 的通知
        let line = json!({"jsonrpc": "2.0", "method": method}).to_string(); // 构造通知行
        use tokio::io::AsyncWriteExt; // 异步写 trait
        let mut stdin = self.stdin.lock().await; // 加锁标准输入
        let _ = stdin.write_all(format!("{line}\n").as_bytes()).await; // 写入通知
        let _ = stdin.flush().await; // 刷送
    }
    async fn close(&self) { // 终止子进程
        let mut child = self.child.lock().await; // 加锁子进程
        let _ = child.kill().await; // 发送终止信号
    }
}
impl HttpTransport { // HTTP/SSE 传输实现
    async fn request(&self, method: &str, params: Value) -> Result<Value, String> { // HTTP 模式发起请求
        let id = self.next.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1; // 分配请求 id
        let payload = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}); // 构造 RPC 请求
        let session = self.session.lock().clone(); // 读取会话 id
        let response = post_json(&self.client, &self.url, &self.headers, &payload, &session).await?; // 发送请求
        if let Some(value) = response.headers().get("Mcp-Session-Id").and_then(|item| item.to_str().ok()) { // 刷新会话 id
            *self.session.lock() = value.to_string(); // 保存新会话 id
        }
        if !response.status().is_success() { return Err(format!("MCP 服务返回 HTTP {}", response.status())); } // HTTP 错误
        let value = response_payload(response).await?; // 提取结果
        if value.get("error").is_some() { return Err(rpc_error(&value)); } // RPC 错误
        Ok(value.get("result").cloned().unwrap_or(Value::Null)) // 返回结果
    }
    async fn request_sse(&self, method: &str, params: Value) -> Result<Value, String> { // SSE 模式发起请求
        let id = self.next.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1; // 分配请求 id
        let endpoint = self.endpoint.lock().clone(); // 读取事件端点
        if endpoint.is_empty() { return Err("MCP SSE 端点未就绪".to_string()); } // 端点未就绪
        let (sender, receiver) = tokio::sync::oneshot::channel(); // 创建应答通道
        self.sse_pending.lock().insert(id, sender); // 登记通道
        let payload = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}); // 构造 RPC 请求
        let response = post_json(&self.client, &endpoint, &self.headers, &payload, "").await?; // 发送到事件端点
        if !response.status().is_success() { self.sse_pending.lock().remove(&id); return Err(format!("MCP SSE 服务返回 HTTP {}", response.status())); } // HTTP 错误时清理通道
        match tokio::time::timeout(std::time::Duration::from_secs(30), receiver).await { // 等待 SSE 帧应答
            Ok(Ok(value)) => { if value.get("error").is_some() { Err(rpc_error(&value)) } else { Ok(value.get("result").cloned().unwrap_or(Value::Null)) } } // 分类处理应答
            _ => { self.sse_pending.lock().remove(&id); Err(format!("MCP SSE 请求超时: {method}")) } // 超时清理并返回错误
        }
    }
}
impl Drop for McpClient { // 客户端析构时终止子进程
    fn drop(&mut self) { // 析构钩子
        if let Transport::Stdio(transport) = &self.transport { // 仅 stdio 传输需清理
            let _ = transport.child.try_lock().map(|mut child| child.start_kill()); // 尝试终止子进程
        }
    }
}
impl McpClient { // 客户端连接与调用
    pub async fn connect(item: &Value) -> Result<Self, String> { // 按配置连接 MCP 服务
        let name = item.get("name").and_then(Value::as_str).unwrap_or_default().to_string(); // 服务名
        let kind = item.get("type").and_then(Value::as_str).unwrap_or("stdio").to_string(); // 传输类型
        let transport = if kind == "stdio" { // stdio 分支：拉起子进程
            let command = item.get("command").and_then(Value::as_str).unwrap_or_default().to_string(); // 启动命令
            let args: Vec<String> = item.get("args").and_then(Value::as_array).map(|items| items.iter().map(|value| crate::state::json_str(Some(value))).collect()).unwrap_or_default(); // 启动参数
            let env = string_map(item.get("env")); // 环境变量
            let mut process = tokio::process::Command::new("cmd"); // 用 cmd 启动
            process.arg("/C").arg(&command).args(&args).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).kill_on_drop(true); // 管道接管输入输出，退出时杀进程
            #[cfg(windows)] process.creation_flags(0x08000000); // CREATE_NO_WINDOW：MCP 子进程不弹出 cmd 黑窗
            for (key, value) in env { process.env(key, value); } // 注入环境变量
            let mut child = process.spawn().map_err(|err| format!("MCP stdio 启动失败: {err}"))?; // 启动子进程
            let stdin = child.stdin.take().ok_or_else(|| "MCP stdin 不可用".to_string())?; // 取标准输入
            let stdout = child.stdout.take().ok_or_else(|| "MCP stdout 不可用".to_string())?; // 取标准输出
            let pending: std::sync::Arc<parking_lot::Mutex<HashMap<u64, tokio::sync::oneshot::Sender<Value>>>> = std::sync::Arc::new(parking_lot::Mutex::new(HashMap::new())); // 应答通道表
            let store = pending.clone(); // 克隆供读线程使用
            tokio::spawn(async move { // 后台读取子进程输出
                use tokio::io::AsyncBufReadExt; // 异步行读 trait
                let mut lines = tokio::io::BufReader::new(stdout).lines(); // 行读取器
                while let Ok(Some(line)) = lines.next_line().await { // 逐行读取
                    let Ok(value) = serde_json::from_str::<Value>(&line) else { continue; }; // 解析 JSON，失败跳过
                    if let Some(id) = value.get("id").and_then(Value::as_u64) { // 取响应 id
                        if let Some(sender) = store.lock().remove(&id) { let _ = sender.send(value); } // 找回通道并发送应答
                    }
                }
            });
            Transport::Stdio(StdioTransport { stdin: tokio::sync::Mutex::new(stdin), child: tokio::sync::Mutex::new(child), pending, next: std::sync::atomic::AtomicU64::new(0) }) // 构造 stdio 传输
        } else { // HTTP/SSE 分支
            let url = item.get("url").and_then(Value::as_str).unwrap_or_default().to_string(); // 服务 URL
            let headers = item.get("headers").and_then(Value::as_object).cloned().unwrap_or_default(); // 请求头
            let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(60)).build().map_err(|err| err.to_string())?; // 构造 HTTP 客户端
            let transport = HttpTransport { client, url: url.clone(), headers, session: parking_lot::Mutex::new(String::new()), next: std::sync::atomic::AtomicU64::new(0), sse_pending: std::sync::Arc::new(parking_lot::Mutex::new(HashMap::new())), endpoint: std::sync::Arc::new(parking_lot::Mutex::new(String::new())), endpoint_ready: std::sync::Arc::new((parking_lot::Mutex::new(false), tokio::sync::Notify::new())) }; // 构造 HTTP 传输
            if kind == "sse" { Transport::Sse(transport) } else { Transport::Http(transport) } // 按类型选择
        };
        let mut client = McpClient { name, tools: Vec::new(), transport }; // 构造客户端
        client.initialize().await?; // await 初始化握手
        client.tools = client.fetch_tools().await?; // await 拉取工具列表
        Ok(client) // 返回就绪客户端
    }
    async fn initialize(&mut self) -> Result<(), String> { // MCP 初始化握手
        if let Transport::Sse(transport) = &self.transport { // SSE 需先建立长连接取端点
            let request = transport.client.get(&transport.url).header("Accept", "text/event-stream"); // 构造 GET 请求
            let response = request.send().await.map_err(|err| format!("MCP SSE 连接失败: {err}"))?; // await 连接
            if !response.status().is_success() { return Err(format!("MCP SSE 连接失败: HTTP {}", response.status())); } // HTTP 错误
            let endpoint = transport.endpoint.clone(); // 共享端点
            let ready = transport.endpoint_ready.clone(); // 共享就绪标志
            let pending = transport.sse_pending.clone(); // 共享应答通道
            tokio::spawn(async move { // 后台读取 SSE 流
                let mut buffer = String::new(); // 缓冲区
                let mut stream = response.bytes_stream(); // 字节流
                use futures_util::StreamExt; // 流扩展 trait
                while let Some(Ok(chunk)) = stream.next().await { // 逐块读取
                    buffer.push_str(&String::from_utf8_lossy(&chunk)); // 追加到缓冲
                    while let Some(index) = buffer.find("\n\n") { // 按空行切事件
                        let block: String = buffer.drain(..index + 2).collect(); // 取出一个事件块
                        let mut event = String::new(); // 事件类型
                        let mut data: Vec<String> = Vec::new(); // 数据行
                        for line in block.lines() { // 遍历事件行
                            if let Some(text) = line.strip_prefix("event:") { event = text.trim().to_string(); } // 取事件类型
                            if let Some(text) = line.strip_prefix("data:") { data.push(text.trim().to_string()); } // 取数据行
                        }
                        if data.is_empty() { continue; } // 无数据跳过
                        let body = data.join("\n"); // 合并数据行
                        if event == "endpoint" { // 端点事件
                            *endpoint.lock() = body; // 保存端点
                            *ready.0.lock() = true; // 标记就绪
                            ready.1.notify_waiters(); // 通知等待者
                            continue; // 处理下一事件
                        }
                        if let Ok(value) = serde_json::from_str::<Value>(&body) { // 解析 JSON-RPC 消息
                            if let Some(id) = value.get("id").and_then(Value::as_u64) { // 取响应 id
                                if let Some(sender) = pending.lock().remove(&id) { let _ = sender.send(value); } // 找回通道并发送应答
                            }
                        }
                    }
                }
            });
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15); // 端点就绪截止时间
            while !*transport.endpoint_ready.0.lock() { // 等待端点就绪
                if tokio::time::Instant::now() >= deadline { return Err("MCP SSE 端点就绪超时".to_string()); } // 超时报错
                tokio::time::sleep(std::time::Duration::from_millis(100)).await; // 轮询等待
            }
        }
        let params = json!({"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "LoveWorldAgent", "version": "0.1.0"}}); // 初始化参数
        let result = self.request("initialize", params).await?; // await 发送初始化请求
        let _ = result; // 初始化结果暂不使用
        if let Transport::Stdio(transport) = &self.transport { transport.notify("notifications/initialized").await; } // stdio 发送已初始化通知
        if let Transport::Sse(transport) = &self.transport { // SSE 发送已初始化通知
            let endpoint = transport.endpoint.lock().clone(); // 取端点
            let payload = json!({"jsonrpc": "2.0", "method": "notifications/initialized"}); // 通知正文
            let _ = post_json(&transport.client, &endpoint, &transport.headers, &payload, "").await; // 发送通知
        }
        Ok(()) // 初始化完成
    }
    async fn request(&self, method: &str, params: Value) -> Result<Value, String> { // 按传输类型分派请求
        match &self.transport { // 传输枚举
            Transport::Stdio(transport) => transport.request(method, params).await, // stdio 请求
            Transport::Http(transport) => transport.request(method, params).await, // HTTP 请求
            Transport::Sse(transport) => transport.request_sse(method, params).await, // SSE 请求
        }
    }
    async fn fetch_tools(&self) -> Result<Vec<McpToolInfo>, String> { // 拉取工具列表
        let result = self.request("tools/list", json!({})).await?; // await 请求工具列表
        let items = result.get("tools").and_then(Value::as_array).cloned().unwrap_or_default(); // 工具数组
        Ok(items.iter().filter_map(|item| { // 逐工具转换
            let name = item.get("name").and_then(Value::as_str).unwrap_or_default().to_string(); // 工具名
            if name.is_empty() { return None; } // 无名跳过
            Some(McpToolInfo { // 构造工具信息
                name, // 工具名
                description: item.get("description").and_then(Value::as_str).unwrap_or_default().to_string(), // 描述
                schema: item.get("inputSchema").cloned().unwrap_or_else(|| json!({"type": "object", "properties": {}})), // 参数 Schema
            })
        }).collect()) // 返回工具列表
    }
    pub async fn call(&self, tool: &str, arguments: &Value) -> Result<String, String> { // 调用 MCP 工具
        let result = self.request("tools/call", json!({"name": tool, "arguments": arguments})).await?; // await 调用
        let mut blocks: Vec<String> = Vec::new(); // 文本块缓冲
        if let Some(items) = result.get("content").and_then(Value::as_array) { // 遍历内容块
            for item in items { // 逐块
                if item.get("type").and_then(Value::as_str) == Some("text") { // 只取文本块
                    blocks.push(item.get("text").and_then(Value::as_str).unwrap_or_default().to_string()); // 收集文本
                }
            }
        }
        let text = if blocks.is_empty() { result.to_string() } else { blocks.join("\n") }; // 无文本块则整体 JSON
        if result.get("isError").and_then(Value::as_bool).unwrap_or(false) { return Err(text); } // 错误结果上抛
        Ok(text) // 返回工具输出
    }
    pub async fn close(&self) { // 关闭客户端
        if let Transport::Stdio(transport) = &self.transport { transport.close().await; } // 仅 stdio 需终止子进程
    }
}
pub async fn connect_all() -> McpRuntime { // 获取常驻运行时快照
    let manager = manager(); // 取管理器
    if !manager.started() { spawn_startup_connect(); } // 首轮连接由启动任务负责，此处只取常驻快照，不阻塞对话
    let snapshot = manager.runtime(); // 读快照
    McpRuntime { clients: snapshot.clients.clone(), catalog: snapshot.catalog.clone() } // 返回副本
}
pub async fn close_all(_runtime: &McpRuntime) { // 兼容旧接口的空实现
    // 常驻运行时由 McpManager 统一持有：会话结束不关闭连接，仅在重连时整体替换
}
pub async fn shutdown_all() { // 应用退出时关闭全部常驻 MCP 客户端
    let snapshot = manager().runtime();
    *manager().runtime.lock() = std::sync::Arc::new(McpRuntime { clients: Vec::new(), catalog: std::collections::BTreeMap::new() });
    for client in &snapshot.clients { client.close().await; }
}
async fn connect_with_retry(item: &Value, retries: bool) -> Option<std::sync::Arc<McpClient>> { // 带重试的连接
    let manager = manager(); // 取管理器
    let name = item.get("name").and_then(Value::as_str).unwrap_or_default().to_string(); // 服务名
    let (_, error) = validate(item); // 校验配置
    if !error.is_empty() { // 配置无效
        manager.set_status(&name, "error", 0, &error); // 标记错误
        crate::utils::logger::warn("mcp", &format!("MCP 配置无效: {name} ({error})")); // 记录告警
        return None; // 放弃连接
    }
    let delays: &[u64] = if retries { &[0, 2, 5, 10] } else { &[0] }; // 重试退避序列
    let mut last = String::new(); // 记录最后一次错误
    for (index, delay) in delays.iter().enumerate() { // 逐次尝试
        if *delay > 0 { tokio::time::sleep(std::time::Duration::from_secs(*delay)).await; } // 退避等待
        manager.set_status(&name, "connecting", 0, ""); // 标记连接中
        match McpClient::connect(item).await { // await 连接
            Ok(client) => { // 连接成功
                let tools = client.tools.len(); // 工具数
                manager.set_status(&name, "connected", tools, ""); // 标记已连接
                crate::utils::logger::info("mcp", &format!("MCP server 已连接（第 {} 次尝试）: {name}，工具 {tools} 个", index + 1)); // 记录成功
                return Some(std::sync::Arc::new(client)); // 返回客户端
            }
            Err(err) => { // 连接失败
                last = err; // 保存错误
                crate::utils::logger::warn("mcp", &format!("MCP server 连接失败（第 {} 次尝试）: {name} ({last})", index + 1)); // 记录失败
            }
        }
    }
    manager.set_status(&name, "error", 0, &last); // 全部失败标记错误
    None // 返回空
}
async fn connect_group(retries: bool) -> std::sync::Arc<McpRuntime> { // 并发连接所有启用的服务
    let manager = manager(); // 取管理器
    let _guard = manager.gate.lock().await; // 持互斥锁，避免重复拉起
    let items: Vec<Value> = load_servers().iter().map(normalize).collect(); // 归一化服务配置
    manager.reset_statuses(&items); // 重置状态
    manager.started.store(true, std::sync::atomic::Ordering::SeqCst); // 标记已启动
    let enabled: Vec<Value> = items.iter().filter(|item| item.get("enabled").and_then(Value::as_bool).unwrap_or(true)).cloned().collect(); // 只保留启用的
    crate::utils::logger::info("mcp", &format!("MCP 常驻连接开始: 已启用 {} 项，共 {} 项，重试={retries}", enabled.len(), items.len())); // 记录开始
    let clients: Vec<std::sync::Arc<McpClient>> = futures_util::future::join_all(enabled.iter().map(|item| connect_with_retry(item, retries))).await.into_iter().flatten().collect(); // 并发连接并收集成功项
    let mut catalog: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new(); // 工具目录
    for client in &clients { catalog.insert(client.name.clone(), client.tools.iter().map(|tool| tool.name.clone()).collect()); } // 逐客户端登记工具
    let runtime = std::sync::Arc::new(McpRuntime { clients, catalog }); // 构造运行时
    *manager.runtime.lock() = runtime.clone(); // 替换常驻快照
    crate::utils::logger::info("mcp", &format!("MCP 常驻连接完成: 成功 {} / 启用 {}", runtime.clients.len(), enabled.len())); // 记录完成
    runtime // 返回运行时
}
pub async fn startup_connect() -> std::sync::Arc<McpRuntime> { // 启动时长连接（带重试）
    connect_group(true).await // 委托连接组
}
pub fn spawn_startup_connect() { // 后台启动首轮连接
    if manager().started() { return; } // 已启动则跳过
    tauri::async_runtime::spawn(async move { let _ = startup_connect().await; }); // 在 tauri 运行时执行
}
pub async fn reconnect_all() -> std::sync::Arc<McpRuntime> { // 重连全部（不重试）
    connect_group(false).await // 委托连接组
}
pub fn spawn_reconnect_all() { // 后台重连全部
    tauri::async_runtime::spawn(async move { let _ = reconnect_all().await; }); // 在 tauri 运行时执行
}
pub fn spawn_reconnect(name: &str) { // 后台重连单个服务
    let name = name.to_string(); // 克隆服务名
    tauri::async_runtime::spawn(async move { let _ = reconnect(&name).await; }); // 在 tauri 运行时执行
}
pub async fn reconnect(name: &str) -> Result<Value, String> { // 重连单个服务
    let manager = manager(); // 取管理器
    let item = load_servers().iter().map(|raw| normalize(raw)).find(|item| item.get("name").and_then(Value::as_str) == Some(name)).ok_or_else(|| "MCP 服务不存在".to_string())?; // 查找服务配置
    if !item.get("enabled").and_then(Value::as_bool).unwrap_or(true) { // 服务已禁用
        manager.drop_client(name); // 移除客户端
        manager.set_status(name, "disabled", 0, "未启用"); // 标记禁用
        return Ok(manager.status_of(name).unwrap_or_else(|| json!({"status": "disabled", "tools": 0, "error": "未启用", "updated_at": now_seconds()}))); // 返回状态
    }
    let _guard = manager.gate.lock().await; // 持互斥锁
    match connect_with_retry(&item, false).await { // 连接（不重试）
        Some(client) => manager.store_client(client), // 成功则写入
        None => manager.drop_client(name), // 失败则移除
    }
    manager.status_of(name).ok_or_else(|| "MCP 状态缺失".to_string()) // 返回最新状态
}
pub fn statuses() -> Value { // 对外暴露全部状态
    manager().statuses() // 委托管理器
}
pub fn status_of(name: &str) -> Option<Value> { // 对外暴露单个状态
    manager().status_of(name) // 委托管理器
}
pub fn runtime() -> std::sync::Arc<McpRuntime> { // 对外暴露运行时
    manager().runtime() // 委托管理器
}
pub fn sanitize_name(text: &str) -> String { // 把名称净化为工具名安全字符
    text.chars().map(|ch| if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' { ch } else { '_' }).collect() // 非法字符替换为下划线
}
pub fn tool_defs(runtime: &McpRuntime) -> Vec<crate::agent::tools::ToolDef> { // 把 MCP 工具转换为内部 ToolDef
    let mut defs: Vec<crate::agent::tools::ToolDef> = Vec::new(); // 结果定义列表
    for client in &runtime.clients { // 遍历客户端
        for tool in &client.tools { // 遍历工具
            let name = format!("mcp_{}__{}", sanitize_name(&client.name), sanitize_name(&tool.name)); // 生成 mcp_<服务>__<工具> 名称
            let client = client.clone(); // 克隆客户端供闭包
            let tool_name = tool.name.clone(); // 克隆工具名供闭包
            let handler: crate::agent::tools::ToolHandler = std::sync::Arc::new(move |args: String| { // 构造工具处理器
                let client = client.clone(); // 每次调用克隆客户端
                let tool_name = tool_name.clone(); // 每次调用克隆工具名
                Box::pin(async move { // 固定异步句柄
                    let value: Value = serde_json::from_str(&args).unwrap_or_else(|_| json!({})); // 解析参数，非法 JSON 按空对象处理
                    match client.call(&tool_name, &value).await { // await 调用 MCP 工具
                        Ok(text) => text, // 成功返回正文
                        Err(err) => { manager().mark_error(&client.name, &err); format!("调用失败：{err}") } // 失败标记错误并返回错误文案
                    }
                })
            });
            defs.push(crate::agent::tools::ToolDef::new(&name, &tool.description, tool.schema.clone(), handler).with_group("mcp")); // 登记工具定义，归入 mcp 审批组
        }
    }
    defs // 返回全部 MCP 工具定义
}
