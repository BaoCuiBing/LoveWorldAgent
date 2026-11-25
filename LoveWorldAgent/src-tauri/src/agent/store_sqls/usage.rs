use rusqlite::Connection; // 会话 token 明细：按会话累加与读取，缺失字段记 0
use serde_json::{json, Map, Value}; // JSON 构造与对象映射
use crate::agent::store_sqls::db::{empty_usage, now, USAGE_KEYS}; // 空用量、时间与字段列表
fn usage_row(conn: &Connection, session_id: &str) -> rusqlite::Result<Value> { // 读取会话用量行
    let mut statement = conn.prepare("SELECT * FROM session_usage WHERE session_id = ?")?; // 准备查询
    let mut rows = statement.query([session_id])?; // 绑定会话 id
    let Some(row) = rows.next()? else { return Ok(empty_usage()); }; // 无记录返回全零用量
    let mut map = Map::new(); // 用量映射
    for key in USAGE_KEYS { // 遍历用量字段
        let value: i64 = row.get::<_, Option<i64>>(key).ok().flatten().unwrap_or(0); // 读取字段，缺失记 0
        map.insert(key.to_string(), json!(value)); // 写入映射
    }
    Ok(Value::Object(map)) // 返回用量对象
}
pub fn add_session_usage(conn: &Connection, session_id: &str, detail: &Value) -> rusqlite::Result<Value> { // 累加会话用量
    let mut values: Vec<i64> = Vec::new(); // 各字段增量
    for key in USAGE_KEYS { // 遍历用量字段
        let raw = detail.get(key).and_then(Value::as_i64).unwrap_or(0); // 取增量，缺失记 0
        values.push(raw.max(0)); // 负值按 0 处理
    }
    conn.execute( // 插入或累加
        "INSERT INTO session_usage (session_id, input, output, cache_read, cache_write, reasoning, total, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(session_id) DO UPDATE SET input=input+excluded.input, output=output+excluded.output, cache_read=cache_read+excluded.cache_read, \
         cache_write=cache_write+excluded.cache_write, reasoning=reasoning+excluded.reasoning, total=total+excluded.total, updated_at=excluded.updated_at", // 冲突时各字段累加
        rusqlite::params![session_id, values[0], values[1], values[2], values[3], values[4], values[5], now()], // 绑定参数
    )?;
    usage_row(conn, session_id) // 返回累加后的用量
}
pub fn get_session_usage(conn: &Connection, session_id: &str) -> rusqlite::Result<Value> { // 读取会话用量
    usage_row(conn, session_id) // 委托行读取
}
