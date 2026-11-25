use std::collections::{HashMap, HashSet}; // 共享应用状态：SQLite 连接、Tauri 窗口句柄、运行取消令牌、并发任务锁、MCP 缓存与接口工具函数
use std::sync::Arc; // 状态跨线程共享
use axum::http::StatusCode; // HTTP 状态码
use axum::Json; // JSON 响应
use once_cell::sync::OnceCell; // 全局单例
use parking_lot::Mutex; // 互斥锁
use rusqlite::Connection; // SQLite 连接
use serde_json::{json, Map, Value}; // JSON 构造
use tokio_util::sync::CancellationToken; // 取消令牌
use crate::agent::mcp::McpRuntime; // MCP 运行时
use crate::agent::store_sqls::db; // 数据库路径与初始化
use crate::rouer::terminal_router::TerminalManager; // 终端管理器
use crate::utils::browser::BrowserManager; // 浏览器管理器
use crate::utils::logger; // 日志输出
#[derive(Clone)] // 状态可克隆共享
pub struct AppState { // 全局应用状态
    pub db: Arc<Mutex<Connection>>, // 单连接共享：所有会话/消息/用量 SQL 走同一把锁
    pub handle: Arc<Mutex<Option<tauri::AppHandle>>>, // 窗口句柄：HTTP 服务先于窗口创建，故用可选槽位
    pub mcp: Arc<Mutex<HashMap<String, Arc<McpRuntime>>>>, // 会话键 → MCP 连接缓存（审批暂停期间保活）
    pub cancels: Arc<Mutex<HashMap<String, CancellationToken>>>, // 会话键 → 取消令牌（停止本轮状态机）
    pub active_tasks: Arc<Mutex<HashSet<String>>>, // 正在执行流式对话或上下文压缩的会话集合（防并发冲突）
    pub terminal: Arc<TerminalManager>, // 终端会话池：用户 12 / 智能体 4，进程内单例
    pub browser: Arc<BrowserManager>, // 内置浏览器 sidecar 与命名管道客户端，进程内单例
}
pub struct SessionTaskGuard { // RAII 会话并发执行守卫：离开作用域或 panic 时自动释放占用
    tasks: Arc<Mutex<HashSet<String>>>, // 占用集合引用
    key: String, // 当前锁定会话键
}
impl Drop for SessionTaskGuard {
    fn drop(&mut self) { self.tasks.lock().remove(&self.key); }
}
pub type ApiResult = (StatusCode, Json<Value>); // 统一接口返回类型
static STATE: OnceCell<AppState> = OnceCell::new(); // 全局状态单例
fn empty_state(db: Arc<Mutex<Connection>>) -> AppState { // 由数据库连接构造空状态
    AppState { db, handle: Arc::new(Mutex::new(None)), mcp: Arc::new(Mutex::new(HashMap::new())), cancels: Arc::new(Mutex::new(HashMap::new())), active_tasks: Arc::new(Mutex::new(HashSet::new())), terminal: crate::rouer::terminal_router::manager(), browser: crate::utils::browser::manager() } // 各字段初始化为空
}
impl AppState { // 状态操作
    pub fn new() -> anyhow::Result<Self> { // 用文件数据库构造状态
        let path = db::db_path(); // 数据库路径
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; } // 确保父目录
        let conn = db::prepare(&path)?; // 打开并初始化
        Ok(empty_state(Arc::new(Mutex::new(conn)))) // 返回状态
    }
    pub(crate) fn memory() -> Self { // 用内存数据库构造状态（兜底与单测）
        let conn = Connection::open_in_memory().expect("内存数据库创建失败"); // 内存库
        db::init(&conn).expect("内存数据库建表失败"); // 建表
        empty_state(Arc::new(Mutex::new(conn))) // 返回状态
    }
    pub fn load_config(&self) -> Value { // 加载配置
        crate::config::load_config() // 委托
    }
    pub fn save_config(&self, data: &Value) -> anyhow::Result<()> { // 保存配置
        crate::config::atomic_write_json(&crate::config::WRITABLE_CONFIG_FILE, data) // 原子写入
    }
    pub fn mcp_runtime(&self, key: &str) -> Option<Arc<McpRuntime>> { // 取会话 MCP 运行时
        self.mcp.lock().get(key).cloned() // 克隆 Arc
    }
    pub fn set_mcp_runtime(&self, key: &str, runtime: Arc<McpRuntime>) { // 缓存会话 MCP 运行时
        self.mcp.lock().insert(key.to_string(), runtime); // 写入缓存
    }
    pub fn take_mcp_runtime(&self, key: &str) -> Option<Arc<McpRuntime>> { // 取走并移除会话 MCP 运行时
        self.mcp.lock().remove(key) // 移除并返回
    }
    pub fn cancel_token(&self, key: &str) -> CancellationToken { // 取或创建会话取消令牌
        self.cancels.lock().entry(key.to_string()).or_insert_with(CancellationToken::new).clone() // 不存在则创建
    }
    pub fn stop(&self, key: &str) { // 取消会话本轮执行
        if let Some(token) = self.cancels.lock().get(key) { token.cancel(); } // 触发取消
    }
    pub fn stop_all(&self) { // 取消全部运行中会话
        for token in self.cancels.lock().values() { token.cancel(); }
    }
    pub fn clear_cancel(&self, key: &str) { // 清理会话取消令牌
        self.cancels.lock().remove(key); // 移除
    }
    pub fn try_acquire_task(&self, key: &str) -> Option<SessionTaskGuard> { // 尝试获取会话独占任务锁，忙返回 None
        let trimmed = key.trim();
        if trimmed.is_empty() { return None; }
        let mut guard = self.active_tasks.lock();
        if !guard.insert(trimmed.to_string()) { return None; }
        Some(SessionTaskGuard { tasks: self.active_tasks.clone(), key: trimmed.to_string() })
    }
    pub fn has_active_task(&self, key: &str) -> bool { self.active_tasks.lock().contains(key.trim()) } // 指定会话是否有在途任务
    pub fn has_active_tasks(&self) -> bool { !self.active_tasks.lock().is_empty() } // 全局是否有在途任务或压缩
}
pub fn shared() -> AppState { // 获取全局状态
    STATE.get_or_init(|| match AppState::new() { // 首次访问初始化
        Ok(state) => state, // 文件库成功
        Err(err) => { // 失败回退内存库
            logger::error("state", &format!("数据库初始化失败，改用内存库: {err}")); // 记录
            AppState::memory() // 内存库
        }
    }).clone() // 返回克隆
}
pub fn set_handle(handle: tauri::AppHandle) { // 保存 Tauri 窗口句柄
    *shared().handle.lock() = Some(handle); // 写入槽位
}
pub fn ok(data: Value) -> ApiResult { // 构造成功响应
    (StatusCode::OK, Json(json!({"code": 200, "msg": "success", "data": data}))) // 包装数据
}
pub fn fail(code: u16, msg: &str) -> ApiResult { // 构造失败响应
    let status = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR); // 状态码转换
    (status, Json(json!({"code": code, "msg": msg}))) // 包装错误
}
pub fn fail_exc(message: &str) -> ApiResult { // 按错误消息推断状态码
    if message.contains("不存在") { fail(404, message) } else { fail(400, message) } // 不存在为 404
}
pub fn body_object(body: &[u8]) -> Value { // 解析请求体为对象
    if body.is_empty() { return json!({}); } // 空体返回空对象
    match serde_json::from_slice::<Value>(body) { // 解析 JSON
        Ok(value) if value.is_object() => value, // 对象直接返回
        _ => json!({}), // 非对象返回空
    }
}
pub fn query_int(map: &HashMap<String, String>, key: &str, default: i64) -> i64 { // 读取查询参数整数
    map.get(key).and_then(|text| text.trim().parse::<i64>().ok()).unwrap_or(default) // 解析失败用默认
}
pub fn as_bool(value: Option<&Value>, default: bool) -> bool { // 把 JSON 值按真值语义转布尔
    match value { // 按类型分派
        None | Some(Value::Null) => default, // 缺省用默认值
        Some(Value::Bool(flag)) => *flag, // 布尔直取
        Some(other) => { // 其他类型转字符串判断
            let text = match other { Value::String(text) => text.clone(), _ => other.to_string() }; // 取文本
            !matches!(text.trim().to_lowercase().as_str(), "false" | "0" | "no" | "off") // 假值列表
        }
    }
}
pub fn json_str(value: Option<&Value>) -> String { // 把 JSON 值转为字符串
    match value { // 按类型分派
        None | Some(Value::Null) => String::new(), // 空返回空串
        Some(Value::String(text)) => text.clone(), // 字符串直取
        Some(other) => other.to_string(), // 其他转 JSON 文本
    }
}
pub fn obj_str(map: &Map<String, Value>, key: &str) -> String { // 读取对象字符串字段
    json_str(map.get(key)) // 委托
}
pub fn collapse(text: &str) -> String { // 压缩空白为单空格
    text.split_whitespace().collect::<Vec<&str>>().join(" ") // 按空白切分拼接
}
pub fn truncate_chars(text: &str, limit: usize) -> String { // 按字符数截断
    text.chars().take(limit).collect() // 取前 limit 个字符
}
