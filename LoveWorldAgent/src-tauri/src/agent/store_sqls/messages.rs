use rusqlite::Connection; // 消息表：直接存取 goose Message JSON，按会话顺序追加、替换、分页与回滚删除
use goose_provider_types::conversation::message::{ActionRequiredData, Message, MessageContent}; // 消息与内容块类型
use goose_provider_types::conversation::Conversation; // 对话容器
use rmcp::model::Role; // 消息角色
use serde_json::{json, Value}; // JSON 构造
use crate::agent::agent_core::goose_session; // 待决动作判定
use crate::agent::store_sqls::db::now; // 时间字符串
use crate::agent::store_sqls::sessions; // 会话状态写回
fn role_text(role: &Role) -> &'static str { // 角色转文本
    match role { Role::User => "user", Role::Assistant => "assistant" } // 用户/助手
}
fn next_seq(conn: &Connection, session_id: &str) -> rusqlite::Result<i64> { // 计算下一条消息序号
    conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM messages WHERE session_id = ?", [session_id], |row| row.get::<_, i64>(0)).map(|value| value + 1) // 最大序号加一
}
fn exists(conn: &Connection, id: &str) -> bool { // 判断消息 id 是否已存在
    conn.query_row("SELECT COUNT(*) FROM messages WHERE id = ?", [id], |row| row.get::<_, i64>(0)).map(|count| count > 0).unwrap_or(false) // 计数大于零
}
pub fn upsert(conn: &Connection, session_id: &str, message: &Message) -> rusqlite::Result<i64> { // 插入或更新消息，返回序号
    let text = serde_json::to_string(message).unwrap_or_else(|_| "{}".to_string()); // 序列化消息
    let id = message.id.clone().unwrap_or_else(|| format!("msg-{}", uuid::Uuid::new_v4().simple())); // 无 id 则生成
    if exists(conn, &id) { // 已存在则更新
        conn.execute("UPDATE messages SET data = ? WHERE id = ?", rusqlite::params![text, id])?; // 更新数据
        return conn.query_row("SELECT seq FROM messages WHERE id = ?", [id], |row| row.get(0)); // 返回原序号
    }
    let seq = next_seq(conn, session_id)?; // 分配新序号
    conn.execute("INSERT INTO messages (id, session_id, seq, data, created_at) VALUES (?, ?, ?, ?, ?)", rusqlite::params![id, session_id, seq, text, now()])?; // 插入消息
    Ok(seq) // 返回新序号
}
pub fn update(conn: &Connection, id: &str, message: &Message) -> rusqlite::Result<()> { // 按 id 更新消息数据
    let text = serde_json::to_string(message).unwrap_or_else(|_| "{}".to_string()); // 序列化消息
    conn.execute("UPDATE messages SET data = ? WHERE id = ?", rusqlite::params![text, id])?; // 执行更新
    Ok(()) // 完成
}
pub fn list(conn: &Connection, session_id: &str) -> rusqlite::Result<Vec<Message>> { // 按序号升序列出会话全部消息
    let mut statement = conn.prepare("SELECT data FROM messages WHERE session_id = ? ORDER BY seq ASC")?; // 准备查询
    let rows = statement.query_map([session_id], |row| row.get::<_, String>(0))?; // 绑定会话 id
    Ok(rows.flatten().filter_map(|text| serde_json::from_str::<Message>(&text).ok()).collect()) // 反序列化，失败丢弃
}
pub fn list_page(conn: &Connection, session_id: &str, after_seq: i64, limit: i64) -> rusqlite::Result<Vec<Value>> { // 分页列出 seq 之后的消息
    let mut statement = conn.prepare("SELECT id, seq, data, created_at FROM messages WHERE session_id = ? AND seq > ? ORDER BY seq ASC LIMIT ?")?; // 准备分页查询
    let rows = statement.query_map(rusqlite::params![session_id, after_seq, limit], |row| { // 绑定参数
        let id: String = row.get(0)?; // 消息 id
        let seq: i64 = row.get(1)?; // 序号
        let text: String = row.get(2)?; // 消息 JSON
        let created: String = row.get(3).unwrap_or_default(); // 创建时间
        let message: Value = serde_json::from_str(&text).unwrap_or_else(|_| json!({})); // 解析消息，失败空对象
        let role = message.get("role").cloned().unwrap_or_else(|| json!("user")); // 取角色
        Ok(json!({"id": id, "session_id": session_id, "seq": seq, "role": role, "message": message, "created_at": created})) // 组装分页项
    })?;
    Ok(rows.flatten().collect()) // 收集结果
}
pub fn count(conn: &Connection, session_id: &str) -> rusqlite::Result<i64> { // 统计会话消息数
    conn.query_row("SELECT COUNT(*) FROM messages WHERE session_id = ?", [session_id], |row| row.get(0)) // 计数查询
}
pub fn replace(conn: &Connection, session_id: &str, messages: &[Message]) -> rusqlite::Result<()> { // 用给定消息整段替换会话消息
    conn.execute("DELETE FROM messages WHERE session_id = ?", [session_id])?; // 先清空
    for message in messages { upsert(conn, session_id, message)?; } // 逐条写入
    Ok(()) // 完成
}
pub fn remove(conn: &Connection, session_id: &str) -> rusqlite::Result<()> { // 删除会话全部消息
    conn.execute("DELETE FROM messages WHERE session_id = ?", [session_id])?; // 执行删除
    Ok(()) // 完成
}
pub fn role_of(message: &Message) -> &'static str { // 取消息角色文本
    role_text(&message.role) // 委托角色转换
}
pub fn is_user_prompt(message: &Message) -> bool { // 判断是否为用户显式提问消息（与前端 isPromptMessage 一致）
    if message.role != Role::User || !message.is_user_visible() { return false; } // 仅可见用户消息可作轮次起点
    let has_text = message.content.iter().any(|block| matches!(block, MessageContent::Text(item) if !item.text.trim().is_empty())); // 需含非空正文
    if !has_text { return false; } // 无正文跳过
    !message.content.iter().any(|block| { // 排除工具回执与审批确认消息
        matches!(block, MessageContent::ToolResponse(_))
            || matches!(block, MessageContent::ActionRequired(action) if matches!(action.data, ActionRequiredData::ToolConfirmationResponse { .. }))
    })
}
pub fn is_compaction_message(message: &Message) -> bool { // 判断是否为独立展示的上下文压缩助手消息
    message.metadata.operation_note("context", "compactionCard").is_some()
        || message.content.iter().any(|block| matches!(block, MessageContent::SystemNotification(note) if note.msg == "context_compaction"))
}
pub fn delete_turn(conn: &Connection, session_id: &str, turn_key: &str, after: bool) -> rusqlite::Result<i64> { // 按轮次起点消息 ID 删除本轮或回滚后续全部轮次
    let seq: Option<i64> = conn.query_row("SELECT seq FROM messages WHERE id = ? AND session_id = ?", rusqlite::params![turn_key, session_id], |row| row.get(0)).ok(); // 查询轮次起始序号
    let Some(start_seq) = seq else { return Ok(0); }; // 未命中返回 0
    if after { // 回滚模式：删除起点及后续全部消息
        let deleted = conn.execute("DELETE FROM messages WHERE session_id = ? AND seq >= ?", rusqlite::params![session_id, start_seq])?; // 批量删除
        return Ok(deleted as i64); // 返回删除行数
    }
    let mut statement = conn.prepare("SELECT seq, data FROM messages WHERE session_id = ? AND seq > ? ORDER BY seq ASC")?; // 顺序扫描后续消息定位下一轮起点
    let rows = statement.query_map(rusqlite::params![session_id, start_seq], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?; // 提取序号与 JSON
    let mut end_seq: Option<i64> = None; // 下一轮起点序号
    for row in rows.flatten() { // 遍历查找下一个用户提问或独立压缩轮
        if let Ok(parsed) = serde_json::from_str::<Message>(&row.1) { // 反序列化消息结构
            if is_user_prompt(&parsed) || is_compaction_message(&parsed) { end_seq = Some(row.0); break; } // 命中下一轮停止
        }
    }
    let deleted = if let Some(limit_seq) = end_seq { // 仅删当前轮至下一轮前
        conn.execute("DELETE FROM messages WHERE session_id = ? AND seq >= ? AND seq < ?", rusqlite::params![session_id, start_seq, limit_seq])? // 区间删除本轮
    } else { // 末轮直接删至末尾
        conn.execute("DELETE FROM messages WHERE session_id = ? AND seq >= ?", rusqlite::params![session_id, start_seq])? // 删除本轮至结尾
    };
    Ok(deleted as i64) // 返回删除行数
}
pub fn is_compacted_hidden(message: &Message) -> bool { // 判断是否因上下文压缩被标记对模型隐藏
    message.metadata.operation_note("context", "compactedHidden").and_then(Value::as_bool).unwrap_or(false) // 仅识别压缩写入的隐藏标记
}
pub fn hide_for_agent(conn: &Connection, session_id: &str, ids: &[String]) -> rusqlite::Result<usize> { // 仅对模型隐藏已压缩旧消息，保留 UI 历史可见性
    if ids.is_empty() { return Ok(0); } // 空列表直接返回
    let mut hidden = 0usize; // 实际更新计数
    for mut message in list(conn, session_id)? { // 遍历会话消息
        let Some(id) = message.id.clone() else { continue; }; // 无 ID 跳过
        if !ids.iter().any(|target| target == &id) { continue; } // 非目标消息跳过
        if !message.metadata.agent_visible { continue; } // 已隐藏跳过
        message.metadata.agent_visible = false; // 仅置模型不可见，保留 user_visible
        message.metadata.set_operation_note("context", "compactedHidden", json!(true)); // 标记为压缩隐藏，区分报错与子智能体消息
        update(conn, &id, &message)?; // 写回数据库
        hidden += 1; // 累加更新数
    }
    Ok(hidden) // 返回隐藏条数
}
pub fn restore_agent_visible(conn: &Connection, session_id: &str) -> rusqlite::Result<usize> { // 回滚失效摘要后恢复被压缩隐藏消息的模型可见性
    let mut restored = 0usize; // 恢复条数
    for mut message in list(conn, session_id)? { // 遍历剩余消息
        let Some(id) = message.id.clone() else { continue; }; // 无 ID 跳过
        if !is_compacted_hidden(&message) { continue; } // 仅恢复因压缩隐藏的消息，不触碰报错或子智能体消息
        message.metadata.agent_visible = true; // 恢复模型可见
        if let Some(ops) = message.metadata.operations.as_mut() { // 清理已恢复消息的压缩隐藏标记
            if let Some(ctx) = ops.get_mut("context") { ctx.remove("compactedHidden"); } // 删除 compactedHidden 键
            if ops.get("context").map(|map| map.is_empty()).unwrap_or(false) { ops.remove("context"); } // 空分组一并清理
        }
        update(conn, &id, &message)?; // 写回数据库
        restored += 1; // 累加恢复数
    }
    Ok(restored) // 返回恢复条数
}
pub fn recompute_session_status(conn: &Connection, session_id: &str) -> rusqlite::Result<&'static str> { // 按剩余消息重算会话真实状态并落库
    let all = list(conn, session_id)?; // 读取剩余消息列表
    let status = if all.is_empty() { // 已删空回退完成态
        "completed" // 空会话默认正常
    } else if goose_session::has_pending(&Conversation::new_unvalidated(all.clone())) { // 仍有未决确认或提问
        "interrupted" // 标记中断态
    } else { // 仅当当前末轮仍带错误块时才判 failed
        let start = all.iter().rposition(|message| is_user_prompt(message) || is_compaction_message(message)).unwrap_or(0); // 定位最后一轮起点（提问或独立压缩）
        let has_error = all[start..].iter().any(|message| message.error_kind().is_some()); // 检测末轮错误块
        if has_error { "failed" } else { "completed" } // 返回准确状态
    };
    let _ = sessions::update_session(conn, session_id, None, Some(status)); // 写回 sessions 表
    Ok(status) // 返回重算状态
}
#[cfg(test)] // 消息轮次删除与状态重算单元测试
mod tests { // 使用内存 SQLite 验证单轮删除、回滚截断、隐藏与状态恢复
    use super::*; // 引入被测函数
    use crate::agent::store_sqls::db; // 建表工具
    use goose_provider_types::conversation::message::MessageErrorKind; // 错误类型
    #[test] // 验证单轮删除、after 回滚及状态从 failed 回退 completed
    fn delete_turn_and_recompute_status_works() {
        let conn = Connection::open_in_memory().unwrap(); // 创建内存库
        db::init(&conn).unwrap(); // 初始化表结构
        let sess = sessions::create_session(&conn, "test", "").unwrap(); // 插入测试会话
        let sid = sess["id"].as_str().unwrap(); // 会话 ID
        let u1 = Message::user().with_text("q1").with_id("m1"); // 第 1 轮用户消息
        let a1 = Message::assistant().with_text("ok1").with_id("m2"); // 第 1 轮助手回复
        let u2 = Message::user().with_text("q2").with_id("m3"); // 第 2 轮用户消息
        let a2 = Message::assistant().with_error(MessageErrorKind::Other, "err2").with_id("m4"); // 第 2 轮报错
        for m in [&u1, &a1, &u2, &a2] { upsert(&conn, sid, m).unwrap(); } // 依次落库
        assert_eq!(recompute_session_status(&conn, sid).unwrap(), "failed"); // 末轮出错应为 failed
        let hidden = hide_for_agent(&conn, sid, &["m1".to_string()]).unwrap(); // 模拟压缩隐藏 m1
        assert_eq!(hidden, 1); // 命中 1 条
        assert!(!list(&conn, sid).unwrap()[0].is_agent_visible()); // 模型不可见但用户仍可见
        assert_eq!(restore_agent_visible(&conn, sid).unwrap(), 1); // 恢复可见性
        assert_eq!(delete_turn(&conn, sid, "m3", true).unwrap(), 2); // 回滚删除第 2 轮（m3+m4）
        assert_eq!(count(&conn, sid).unwrap(), 2); // 仅剩第 1 轮 2 条
        assert_eq!(recompute_session_status(&conn, sid).unwrap(), "completed"); // 末轮恢复 normal 应回退 completed
    }
}
