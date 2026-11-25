use rusqlite::Connection; // 会话表 CRUD：列表/取单个/新建/更新状态/删除（级联清理消息、轮次、用量与上下文）
use serde_json::{json, Value}; // JSON 构造
use uuid::Uuid; // 会话 id 生成
use crate::agent::store_sqls::db::{now, session, SESSION_COLUMNS}; // 时间、行转换与列定义
pub fn list_sessions(conn: &Connection, q: &str) -> rusqlite::Result<Vec<Value>> { // 列出会话，可按关键字过滤标题或 id
    let sql = format!("SELECT {SESSION_COLUMNS} FROM sessions ORDER BY updated_at DESC"); // 按更新时间倒序
    let mut statement = conn.prepare(&sql)?; // 准备查询
    let rows = statement.query_map([], |row| session(row))?; // 行转会话 JSON
    let items: Vec<Value> = rows.flatten().collect(); // 收集结果
    let keyword = q.trim().to_lowercase(); // 归一化关键字
    if keyword.is_empty() { return Ok(items); } // 无关键字直接返回
    Ok(items.into_iter().filter(|item| { // 按标题或 id 过滤
        let title = item.get("title").and_then(Value::as_str).unwrap_or_default().to_lowercase(); // 标题
        let id = item.get("id").and_then(Value::as_str).unwrap_or_default().to_lowercase(); // id
        title.contains(&keyword) || id.contains(&keyword) // 任一命中即保留
    }).collect())
}
pub fn get_session(conn: &Connection, session_id: &str) -> rusqlite::Result<Option<Value>> { // 按 id 查询单个会话
    let sql = format!("SELECT {SESSION_COLUMNS} FROM sessions WHERE id = ?"); // 查询语句
    let mut statement = conn.prepare(&sql)?; // 准备查询
    let mut rows = statement.query([session_id])?; // 绑定 id
    match rows.next()? { // 取第一条
        Some(row) => Ok(Some(session(row)?)), // 命中转 JSON
        None => Ok(None), // 无记录
    }
}
pub fn create_session(conn: &Connection, title: &str, project_path: &str) -> rusqlite::Result<Value> { // 新建会话
    let stamp = now(); // 当前时间
    let id = format!("s{}", &Uuid::new_v4().simple().to_string()[..12]); // 生成会话 id
    let item = json!({"id": id, "title": title, "status": "running", "project_path": project_path, "created_at": stamp, "updated_at": stamp}); // 会话对象
    conn.execute("INSERT INTO sessions (id, title, status, project_path, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)", rusqlite::params![id, title, "running", project_path, stamp, stamp])?; // 插入数据库
    Ok(item) // 返回会话对象
}
pub fn update_session(conn: &Connection, session_id: &str, title: Option<&str>, status: Option<&str>) -> rusqlite::Result<Option<Value>> { // 更新会话标题或状态
    let existed: Option<String> = conn.query_row("SELECT id FROM sessions WHERE id = ?", [session_id], |row| row.get(0)).ok(); // 校验存在
    if existed.is_none() { return Ok(None); } // 不存在返回 None
    let mut fields: Vec<&str> = Vec::new(); // 待更新字段
    let mut values: Vec<String> = Vec::new(); // 绑定值
    if let Some(text) = title { fields.push("title = ?"); values.push(text.to_string()); } // 更新标题
    if let Some(text) = status { fields.push("status = ?"); values.push(text.to_string()); } // 更新状态
    fields.push("updated_at = ?"); // 总是更新时间
    values.push(now()); // 时间值
    let sql = format!("UPDATE sessions SET {} WHERE id = ?", fields.join(", ")); // 拼接 SQL
    values.push(session_id.to_string()); // 绑定会话 id
    conn.execute(&sql, rusqlite::params_from_iter(values.iter()))?; // 执行更新
    get_session(conn, session_id) // 返回更新后的会话
}
pub fn delete_session(conn: &Connection, session_id: &str) -> rusqlite::Result<bool> { // 删除会话及其关联数据
    let existed: Option<String> = conn.query_row("SELECT id FROM sessions WHERE id = ?", [session_id], |row| row.get(0)).ok(); // 校验存在
    if existed.is_none() { return Ok(false); } // 不存在返回 false
    conn.execute("DELETE FROM messages WHERE session_id = ?", [session_id])?; // 清理消息
    conn.execute("DELETE FROM turns WHERE session_id = ?", [session_id])?; // 清理轮次
    conn.execute("DELETE FROM session_usage WHERE session_id = ?", [session_id])?; // 清理用量
    conn.execute("DELETE FROM session_context WHERE session_id = ?", [session_id])?; // 清理上下文
    conn.execute("DELETE FROM session_compaction WHERE session_id = ?", [session_id])?; // 清理压缩基线状态
    conn.execute("DELETE FROM context_facts WHERE session_id = ?", [session_id])?; // 清理关键事实
    conn.execute("DELETE FROM sessions WHERE id = ?", [session_id])?; // 删除会话本身
    Ok(true) // 删除成功
}
