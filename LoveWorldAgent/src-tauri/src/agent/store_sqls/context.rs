use rusqlite::Connection; // 压缩摘要与关键事实落盘：摘要每会话保留最新一份，事实去重追加
use serde_json::{json, Value}; // JSON 构造
use uuid::Uuid; // 事实 id 生成
use crate::agent::store_sqls::db::now; // 时间字符串
pub fn save_context(conn: &Connection, session_id: &str, summary: &str, tokens_before: i64, tokens_after: i64) -> rusqlite::Result<Value> { // 保存会话摘要（同会话覆盖）
    conn.execute( // 插入或更新摘要
        "INSERT INTO session_context (session_id, summary, tokens_before, tokens_after, updated_at) VALUES (?, ?, ?, ?, ?) \
         ON CONFLICT(session_id) DO UPDATE SET summary=excluded.summary, tokens_before=excluded.tokens_before, tokens_after=excluded.tokens_after, updated_at=excluded.updated_at", // 冲突时覆盖摘要与 token 统计
        rusqlite::params![session_id, summary, tokens_before, tokens_after, now()], // 绑定参数
    )?;
    Ok(json!({"session_id": session_id, "summary": summary, "tokens_before": tokens_before, "tokens_after": tokens_after})) // 返回写入结果
}
pub fn load_context(conn: &Connection, session_id: &str) -> rusqlite::Result<Option<Value>> { // 读取会话摘要
    let mut statement = conn.prepare("SELECT summary, tokens_before, tokens_after, updated_at FROM session_context WHERE session_id = ?")?; // 准备查询
    let mut rows = statement.query([session_id])?; // 绑定会话 id
    let Some(row) = rows.next()? else { return Ok(None); }; // 无记录返回 None
    Ok(Some(json!({
        "summary": row.get::<_, Option<String>>("summary").ok().flatten().unwrap_or_default(), // 摘要正文
        "tokens_before": row.get::<_, Option<i64>>("tokens_before").ok().flatten().unwrap_or(0), // 压缩前 token
        "tokens_after": row.get::<_, Option<i64>>("tokens_after").ok().flatten().unwrap_or(0), // 压缩后 token
        "updated_at": row.get::<_, Option<String>>("updated_at").ok().flatten().unwrap_or_default(), // 更新时间
    })))
}
pub fn add_facts(conn: &Connection, session_id: &str, facts: &[Value]) -> rusqlite::Result<Vec<String>> { // 去重追加关键事实
    let mut added: Vec<String> = Vec::new(); // 新增事实列表
    for raw in facts { // 遍历事实
        let text = crate::state::json_str(Some(raw)); // 转为文本
        let fact = crate::state::truncate_chars(&crate::state::collapse(&text), 300); // 折叠空白并截断
        if fact.is_empty() { continue; } // 空事实跳过
        let existed: Option<String> = conn.query_row("SELECT id FROM context_facts WHERE session_id = ? AND fact = ?", rusqlite::params![session_id, fact], |row| row.get(0)).ok(); // 查重
        if existed.is_some() { continue; } // 已存在跳过
        conn.execute("INSERT INTO context_facts (id, session_id, fact, created_at) VALUES (?, ?, ?, ?)", rusqlite::params![format!("f{}", &Uuid::new_v4().simple().to_string()[..12]), session_id, fact, now()])?; // 插入新事实
        added.push(fact); // 记录新增
    }
    Ok(added) // 返回新增事实
}
pub fn load_facts(conn: &Connection, session_id: &str, limit: i64) -> rusqlite::Result<Vec<String>> { // 读取最近的关键事实
    let mut statement = conn.prepare("SELECT fact FROM context_facts WHERE session_id = ? ORDER BY created_at DESC, rowid DESC LIMIT ?")?; // 按时间倒序查询
    let rows = statement.query_map(rusqlite::params![session_id, limit.max(0)], |row| row.get::<_, Option<String>>(0))?; // 绑定会话与上限
    Ok(rows.flatten().flatten().filter(|text| !text.is_empty()).collect()) // 过滤空事实后收集
}
pub fn get_compaction_state(conn: &Connection, session_id: &str) -> (i64, i64) { // 读取自动压缩累计基线与连续失败计数
    conn.query_row("SELECT token_baseline, consecutive_failures FROM session_compaction WHERE session_id = ?", [session_id], |row| Ok((row.get::<_, i64>(0).unwrap_or(0), row.get::<_, i64>(1).unwrap_or(0)))).unwrap_or((0, 0)) // 未记录返回 (0, 0)
}
pub fn set_compaction_baseline(conn: &Connection, session_id: &str, baseline: i64) -> rusqlite::Result<()> { // 更新压缩基线并清零断路器失败次数
    conn.execute( // 插入或覆盖基线记录
        "INSERT INTO session_compaction (session_id, token_baseline, consecutive_failures, updated_at) VALUES (?, ?, 0, ?) \
         ON CONFLICT(session_id) DO UPDATE SET token_baseline=excluded.token_baseline, consecutive_failures=0, updated_at=excluded.updated_at", // 成功后复位失败次数
        rusqlite::params![session_id, baseline.max(0), now()], // 绑定参数
    )?;
    Ok(()) // 完成
}
pub fn record_compaction_failure(conn: &Connection, session_id: &str) -> rusqlite::Result<i64> { // 记录压缩失败并自增连续失败计数（断路器）
    conn.execute( // 插入或累加失败次数
        "INSERT INTO session_compaction (session_id, token_baseline, consecutive_failures, updated_at) VALUES (?, 0, 1, ?) \
         ON CONFLICT(session_id) DO UPDATE SET consecutive_failures=consecutive_failures+1, updated_at=excluded.updated_at", // 连续失败加一
        rusqlite::params![session_id, now()], // 绑定参数
    )?;
    Ok(get_compaction_state(conn, session_id).1) // 返回最新失败计数
}
pub fn reset_compaction(conn: &Connection, session_id: &str, baseline: i64) -> rusqlite::Result<()> { // 回滚后清理已过期摘要、事实并重设压缩基线
    conn.execute("DELETE FROM session_context WHERE session_id = ?", [session_id])?; // 清除历史摘要
    conn.execute("DELETE FROM context_facts WHERE session_id = ?", [session_id])?; // 清除关键事实
    set_compaction_baseline(conn, session_id, baseline) // 重设基线并清零失败计数
}
