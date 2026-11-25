use rusqlite::Connection; // 轮次表：一轮同时承载 user 与 assistant，支持幂等追加、局部更新与回滚删除
use serde_json::Value; // JSON 动态值
use uuid::Uuid; // 轮次 id 生成
use crate::agent::store_sqls::db::{now, turn}; // 时间与行转换
fn text_of(value: &Value) -> String { // 轮次内容序列化为文本
    match value { Value::String(text) => text.clone(), other => serde_json::to_string(other).unwrap_or_else(|_| "{}".to_string()) } // 字符串直取，其他序列化
}
pub fn list_turns(conn: &Connection, session_id: &str, after_seq: i64, limit: i64) -> rusqlite::Result<Vec<Value>> { // 分页列出 seq 之后的轮次
    let mut statement = conn.prepare("SELECT * FROM turns WHERE session_id = ? AND seq > ? ORDER BY seq ASC LIMIT ?")?; // 准备分页查询
    let rows = statement.query_map(rusqlite::params![session_id, after_seq, limit], |row| turn(row))?; // 绑定参数并转 JSON
    Ok(rows.flatten().collect()) // 收集结果
}
pub fn count_turns(conn: &Connection, session_id: &str) -> rusqlite::Result<i64> { // 统计会话轮次数
    conn.query_row("SELECT COUNT(*) FROM turns WHERE session_id = ?", [session_id], |row| row.get(0)) // 计数查询
}
pub fn append_turn(conn: &Connection, session_id: &str, user: &Value, assistant: &Value, turn_key: Option<&str>) -> rusqlite::Result<Value> { // 幂等追加轮次（有 key 则更新）
    let key = turn_key.map(str::to_string).filter(|text| !text.is_empty()).unwrap_or_else(|| format!("turn-{}", &Uuid::new_v4().simple().to_string()[..12])); // 用传入 key 或生成
    let stamp = now(); // 当前时间
    let existed: Option<String> = conn.query_row("SELECT id FROM turns WHERE id = ? AND session_id = ?", rusqlite::params![key, session_id], |row| row.get(0)).ok(); // 查是否已存在
    if existed.is_some() { // 已存在则更新内容
        conn.execute("UPDATE turns SET user = ?, assistant = ?, updated_at = ? WHERE id = ? AND session_id = ?", rusqlite::params![text_of(user), text_of(assistant), stamp, key, session_id])?; // 更新用户与助手内容
    } else { // 不存在则新增
        let seq: i64 = conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM turns WHERE session_id = ?", [session_id], |row| row.get(0))?; // 取最大序号
        conn.execute("INSERT INTO turns (id, session_id, seq, user, assistant, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)", rusqlite::params![key, session_id, seq + 1, text_of(user), text_of(assistant), stamp, stamp])?; // 插入轮次
    }
    query_turn(conn, &key) // 返回轮次数据
}
fn query_turn(conn: &Connection, key: &str) -> rusqlite::Result<Value> { // 按 id 查询轮次
    let mut statement = conn.prepare("SELECT * FROM turns WHERE id = ?")?; // 准备查询
    let mut rows = statement.query([key])?; // 绑定 id
    match rows.next()? { // 取第一条
        Some(row) => turn(row), // 转 JSON
        None => Ok(serde_json::json!({})), // 无记录返回空对象
    }
}
pub fn update_turn(conn: &Connection, session_id: &str, turn_key: &str, assistant: Option<&Value>, user: Option<&Value>) -> rusqlite::Result<Option<Value>> { // 局部更新轮次内容
    let existed: Option<String> = conn.query_row("SELECT id FROM turns WHERE id = ? AND session_id = ?", rusqlite::params![turn_key, session_id], |row| row.get(0)).ok(); // 校验存在
    if existed.is_none() { return Ok(None); } // 不存在返回 None
    let mut fields: Vec<&str> = Vec::new(); // 待更新字段
    let mut values: Vec<String> = Vec::new(); // 绑定值
    if let Some(value) = user { fields.push("user = ?"); values.push(text_of(value)); } // 更新用户内容
    if let Some(value) = assistant { fields.push("assistant = ?"); values.push(text_of(value)); } // 更新助手内容
    fields.push("updated_at = ?"); // 总是更新时间
    values.push(now()); // 时间值
    let sql = format!("UPDATE turns SET {} WHERE id = ? AND session_id = ?", fields.join(", ")); // 拼接 SQL
    values.push(turn_key.to_string()); // 绑定轮次 id
    values.push(session_id.to_string()); // 绑定会话 id
    conn.execute(&sql, rusqlite::params_from_iter(values.iter()))?; // 执行更新
    query_turn(conn, turn_key).map(Some) // 返回更新后的轮次
}
pub fn delete_turns(conn: &Connection, session_id: &str, turn_key: &str, after: bool) -> rusqlite::Result<i64> { // 删除轮次，after=true 时连后续一起删
    let seq: Option<i64> = conn.query_row("SELECT seq FROM turns WHERE id = ? AND session_id = ?", rusqlite::params![turn_key, session_id], |row| row.get(0)).ok(); // 取轮次序号
    let Some(seq) = seq else { return Ok(0); }; // 不存在返回 0
    let deleted = if after { // 回滚模式：删除本轮及之后
        conn.execute("DELETE FROM turns WHERE session_id = ? AND seq >= ?", rusqlite::params![session_id, seq])?
    } else { // 普通模式：只删本轮
        conn.execute("DELETE FROM turns WHERE id = ? AND session_id = ?", rusqlite::params![turn_key, session_id])?
    };
    Ok(deleted as i64) // 返回删除条数
}
