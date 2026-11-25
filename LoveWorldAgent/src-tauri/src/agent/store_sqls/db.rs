use std::path::{Path, PathBuf}; // store_sqls 公共层：库路径/旧库备份重建、goose 消息表结构、时间格式与行转 JSON 工具
use std::time::{SystemTime, UNIX_EPOCH}; // 时间戳转换
use chrono::{Local, TimeZone}; // 本地时间格式化
use rusqlite::{Connection, Row}; // SQLite 连接与行
use serde_json::{json, Value}; // JSON 构造
use crate::config; // 可写目录配置
pub const SESSION_COLUMNS: &str = "id, title, status, project_path, created_at, updated_at"; // 会话表查询列
pub const USAGE_KEYS: [&str; 6] = ["input", "output", "cache_read", "cache_write", "reasoning", "total"]; // 用量字段列表
pub const LEGACY_BACKUP: &str = "lwa.db.bak"; // 旧版库备份文件名
pub fn db_path() -> PathBuf { // 返回数据库文件路径
    config::writable_static_dir().join("data").join("lwa.db") // 位于可写 static/data 下
}
pub fn now() -> String { // 当前本地时间字符串
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string() // 格式化为年-月-日 时:分:秒
}
pub fn stamp_of(time: SystemTime) -> String { // SystemTime 转本地时间字符串
    let moment = time.duration_since(UNIX_EPOCH).map(|value| value.as_secs() as i64).unwrap_or(0); // 转 Unix 秒
    Local.timestamp_opt(moment, 0).single().unwrap_or_else(Local::now).format("%Y-%m-%d %H:%M:%S").to_string() // 格式化，失败用当前时间
}
fn column_exists(conn: &Connection, table: &str, name: &str) -> bool { // 判断表是否含某列
    let Ok(mut statement) = conn.prepare(&format!("PRAGMA table_info({table})")) else { return false; }; // 查询表结构，失败返回 false
    let Ok(rows) = statement.query_map([], |row| row.get::<_, String>(1)) else { return false; }; // 取列名，失败返回 false
    let found = rows.flatten().any(|item| item == name); // 是否包含目标列
    found // 返回结果
}
fn is_legacy(path: &Path) -> bool { // 判断是否为旧版库（messages 表无 data 列）
    if !path.exists() { return false; } // 文件不存在不是旧库
    let Ok(conn) = Connection::open(path) else { return false; }; // 打开数据库失败按非旧库处理
    let existed: i64 = conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'messages'", [], |row| row.get(0)).unwrap_or(0); // 查 messages 表是否存在
    existed > 0 && !column_exists(&conn, "messages", "data") // 表存在且缺 data 列即为旧库
}
fn backup_legacy(path: &Path) { // 备份并移除旧版库文件
    let target = path.with_file_name(LEGACY_BACKUP); // 备份目标路径
    let _ = std::fs::remove_file(&target); // 先删除旧备份
    if std::fs::rename(path, &target).is_err() { // 优先重命名
        if std::fs::copy(path, &target).is_ok() { let _ = std::fs::remove_file(path); } // 失败则复制后删除原文件
    }
    for suffix in ["-wal", "-shm"] { let _ = std::fs::remove_file(path.with_file_name(format!("lwa.db{suffix}"))); } // 清理 WAL/SHM 附属文件
}
pub fn prepare(path: &Path) -> rusqlite::Result<Connection> { // 打开并初始化数据库
    if is_legacy(path) { backup_legacy(path); } // 旧库先备份再重建
    let conn = Connection::open(path)?; // 打开数据库
    init(&conn)?; // 建表与索引
    Ok(conn) // 返回连接
}
pub fn init(conn: &Connection) -> rusqlite::Result<()> { // 建表与索引（幂等）
    conn.execute("CREATE TABLE IF NOT EXISTS sessions (id TEXT PRIMARY KEY, title TEXT, status TEXT, project_path TEXT, created_at TEXT, updated_at TEXT)", [])?; // 会话表
    conn.execute("CREATE TABLE IF NOT EXISTS messages (id TEXT PRIMARY KEY, session_id TEXT, seq INTEGER, data TEXT, created_at TEXT)", [])?; // 消息表
    conn.execute("CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_session_seq ON messages(session_id, seq)", [])?; // 消息会话序号唯一索引
    conn.execute("CREATE TABLE IF NOT EXISTS turns (id TEXT PRIMARY KEY, session_id TEXT, seq INTEGER, user TEXT, assistant TEXT, created_at TEXT, updated_at TEXT)", [])?; // 轮次表
    conn.execute("CREATE UNIQUE INDEX IF NOT EXISTS idx_turns_session_seq ON turns(session_id, seq)", [])?; // 轮次会话序号唯一索引
    conn.execute("CREATE TABLE IF NOT EXISTS session_usage (session_id TEXT PRIMARY KEY, input INTEGER DEFAULT 0, output INTEGER DEFAULT 0, cache_read INTEGER DEFAULT 0, cache_write INTEGER DEFAULT 0, reasoning INTEGER DEFAULT 0, total INTEGER DEFAULT 0, updated_at TEXT)", [])?; // 用量表
    conn.execute("CREATE TABLE IF NOT EXISTS session_context (session_id TEXT PRIMARY KEY, summary TEXT, tokens_before INTEGER DEFAULT 0, tokens_after INTEGER DEFAULT 0, updated_at TEXT)", [])?; // 上下文摘要表
    conn.execute("CREATE TABLE IF NOT EXISTS session_compaction (session_id TEXT PRIMARY KEY, token_baseline INTEGER DEFAULT 0, consecutive_failures INTEGER DEFAULT 0, updated_at TEXT)", [])?; // 自动压缩基线与断路器表
    conn.execute("CREATE TABLE IF NOT EXISTS context_facts (id TEXT PRIMARY KEY, session_id TEXT, fact TEXT, created_at TEXT)", [])?; // 关键事实表
    conn.execute("CREATE INDEX IF NOT EXISTS idx_context_facts_session ON context_facts(session_id)", [])?; // 事实会话索引
    Ok(()) // 初始化完成
}
pub fn parts(value: Option<String>) -> Value { // 解析 JSON 文本字段，失败按字符串回退
    let Some(text) = value else { return json!(null); }; // 空值返回 null
    match serde_json::from_str::<Value>(&text) { Ok(parsed) => parsed, Err(_) => Value::String(text) } // 解析成功用对象，失败保留原文
}
pub fn detail(value: Option<String>) -> Value { // 解析 JSON 对象字段，失败按空对象回退
    let Some(text) = value else { return json!({}); }; // 空值返回空对象
    match serde_json::from_str::<Value>(&text) { Ok(Value::Object(map)) => Value::Object(map), _ => json!({}) } // 仅接受对象
}
pub fn empty_usage() -> Value { // 生成全零用量对象
    let mut map = serde_json::Map::new(); // 用量映射
    for key in USAGE_KEYS { map.insert(key.to_string(), json!(0)); } // 各字段置零
    Value::Object(map) // 返回对象
}
fn text(row: &Row, key: &str) -> String { // 从行读取字符串列，缺省空串
    row.get::<_, Option<String>>(key).ok().flatten().unwrap_or_default() // 读取可选字符串
}
pub fn session(row: &Row) -> rusqlite::Result<Value> { // 行转会话 JSON
    Ok(json!({
        "id": text(row, "id"), // 会话 id
        "title": text(row, "title"), // 标题
        "status": text(row, "status"), // 状态
        "project_path": text(row, "project_path"), // 项目路径
        "created_at": text(row, "created_at"), // 创建时间
        "updated_at": text(row, "updated_at"), // 更新时间
    }))
}
pub fn turn(row: &Row) -> rusqlite::Result<Value> { // 行转轮次 JSON
    let user = parts(row.get::<_, Option<String>>("user").ok().flatten()); // 解析用户消息
    let assistant = parts(row.get::<_, Option<String>>("assistant").ok().flatten()); // 解析助手消息
    let id = text(row, "id"); // 轮次 id
    let seq = row.get::<_, i64>("seq").unwrap_or(0); // 轮次序号
    Ok(json!({
        "turn_key": id, // 轮次键（下划线）
        "turnKey": id, // 轮次键（驼峰，前端用）
        "session_id": text(row, "session_id"), // 会话 id
        "seq": seq, // 序号
        "ordinal": seq, // 序号别名
        "user": if user.is_object() { user } else { json!({}) }, // 用户消息非对象则空对象
        "assistant": if assistant.is_object() { assistant } else { json!({}) }, // 助手消息非对象则空对象
        "created_at": text(row, "created_at"), // 创建时间
        "updated_at": text(row, "updated_at"), // 更新时间
    }))
}
