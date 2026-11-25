use std::collections::HashMap; // goose 会话适配：Message/Conversation 落 SQLite，状态机 Loader/EffectHandler 与暂停确认读写
use std::sync::Arc; // 数据库连接跨线程共享
use anyhow::Result; // 状态机异步结果类型
use async_trait::async_trait; // 异步 trait 宏
use goose_agent::inference::InferenceEffect; // 推理用量记账效果 trait
use goose_agent::machine::{EffectHandler, MachineSession, SessionLoader}; // 状态机会话与效果处理 trait
use goose_agent::operation::{ConversationEffect, Emitter, MachineEffect}; // 对话变更与事件类型
use goose_provider_types::conversation::message::{ActionRequiredData, Message, MessageContent}; // 消息与审批内容块类型
use goose_provider_types::conversation::token_usage::ProviderUsage; // 模型用量类型
use goose_provider_types::conversation::Conversation; // 对话容器类型
use goose_provider_types::permission::Permission; // 审批权限值
use goose_provider_types::thinking::ThinkingEffort; // 思考强度
use parking_lot::Mutex; // SQLite 连接互斥锁
use rusqlite::Connection; // SQLite 连接类型
use serde_json::{json, Value}; // JSON 构造与动态值
use crate::agent::store_sqls::{messages, usage as usage_store}; // 消息与用量数据库读写
#[derive(Clone)] // 会话快照可克隆
pub struct GooseSession { // 一次状态机读取的会话快照：id + 对话 + 思考强度
    id: String, // 会话 id
    conversation: Conversation, // 对话快照
    thinking: Option<ThinkingEffort>, // 思考强度设置
}
impl GooseSession { // 会话快照构造与访问
    pub fn new(id: &str, conversation: Conversation, thinking: Option<ThinkingEffort>) -> Self { // 构造会话快照
        Self { id: id.to_string(), conversation, thinking } // 保存 id、对话与思考强度
    }
    pub fn conversation_clone(&self) -> Conversation { // 返回对话副本供状态机读取
        self.conversation.clone() // 克隆对话
    }
    pub fn set_conversation(&mut self, conversation: Conversation) { // 用新对话覆盖快照
        self.conversation = conversation; // 替换对话
    }
}
impl MachineSession for GooseSession { // 实现状态机会话接口
    fn id(&self) -> &str { // 返回会话 id
        &self.id // 会话 id 引用
    }
    fn conversation(&self) -> Option<&Conversation> { // 返回当前对话引用
        Some(&self.conversation) // 快照总是有对话
    }
    fn thinking_effort(&self) -> Option<ThinkingEffort> { // 返回思考强度设置
        self.thinking // 直接返回字段
    }
}
pub enum ChatEffect { // 状态机效果：对话变更 + 用量记账
    Conversation(ConversationEffect), // 对话变更效果
    Usage(ProviderUsage), // 用量记账效果
}
impl From<Message> for ChatEffect { // 消息转为追加效果
    fn from(message: Message) -> Self { // 由单条消息构造效果
        ChatEffect::Conversation(ConversationEffect::AppendMessage(message)) // 包装为追加消息
    }
}
impl From<Conversation> for ChatEffect { // 对话转为替换效果
    fn from(conversation: Conversation) -> Self { // 由整段对话构造效果
        ChatEffect::Conversation(ConversationEffect::ReplaceConversation(conversation)) // 包装为替换对话
    }
}
impl MachineEffect for ChatEffect { // 实现机器效果接口
    fn ensure_message_ids(&mut self) { // 为对话效果补齐消息 id
        if let ChatEffect::Conversation(effect) = self { effect.ensure_message_ids(); } // 仅对话效果需要补 id
    }
}
impl InferenceEffect for ChatEffect { // 实现推理用量效果
    fn record_usage(usage: ProviderUsage) -> Self { // 用量记账效果由模型用量构造
        ChatEffect::Usage(usage) // 包装为用量效果
    }
}
pub struct SessionStore { // SQLite 会话读写：状态机每步重新载入，效果落库
    pub db: Arc<Mutex<Connection>>, // 共享数据库连接
    pub session_id: String, // 当前会话 id
    pub thinking: Option<ThinkingEffort>, // 思考强度
}
impl SessionStore { // 会话存储构造
    pub fn new(db: Arc<Mutex<Connection>>, session_id: &str, thinking: Option<ThinkingEffort>) -> Self { // 构造 SQLite 会话存储
        Self { db, session_id: session_id.to_string(), thinking } // 保存连接、会话 id 与思考强度
    }
}
#[async_trait] // 会话加载器异步实现
impl SessionLoader<GooseSession> for SessionStore { // 状态机每步从数据库载入会话
    async fn load(&self, session_id: &str) -> Result<GooseSession> { // 从数据库载入对话
        let conn = self.db.lock(); // 加锁读取 SQLite 连接
        let conversation = load_conversation(&conn, session_id)?; // 从 SQLite 载入对话历史
        Ok(GooseSession::new(session_id, conversation, self.thinking)) // 构造会话快照
    }
}
#[async_trait] // 效果处理器异步实现
impl EffectHandler<GooseSession, ChatEffect> for SessionStore { // 状态机效果落库
    async fn apply_effects(&self, _session: &GooseSession, effects: &mut [ChatEffect], _emit: &Emitter) -> Result<()> { // 将本步效果依次落库
        let conn = self.db.lock(); // 加锁读取 SQLite 连接
        for effect in effects.iter_mut() { // 遍历所有效果
            match effect { // 按效果类型分派
                ChatEffect::Usage(usage) => { let _ = usage_store::add_session_usage(&conn, &self.session_id, &usage_json(usage)); } // 用量写入数据库，失败忽略
                ChatEffect::Conversation(item) => match item { // 对话变更再细分
                    ConversationEffect::AppendMessage(message) => { append_merged(&conn, &self.session_id, message)?; } // 文本增量按同 id 合并后落库
                    ConversationEffect::ReplaceConversation(conversation) => { messages::replace(&conn, &self.session_id, conversation.messages())?; } // 整段对话替换落库
                    ConversationEffect::PatchToolRequestMeta { tool_call_id, patch } => { patch_tool_request(&conn, &self.session_id, tool_call_id, patch)?; } // 补丁工具请求元数据
                    ConversationEffect::SetMessageOperationNote { message_id, operation, key, value } => { set_note(&conn, &self.session_id, message_id, operation, key, value)?; } // 写入消息操作备注
                    ConversationEffect::SetMessageVisibility { message_id, user_visible, agent_visible } => { set_visibility(&conn, &self.session_id, message_id, *user_visible, *agent_visible)?; } // 更新消息可见性
                },
            }
        }
        Ok(()) // 所有效果落库成功
    }
}
pub fn load_conversation(conn: &Connection, session_id: &str) -> rusqlite::Result<Conversation> { // 从消息表构造对话
    Ok(Conversation::new_unvalidated(messages::list(conn, session_id)?)) // 不校验直接构造对话
}
fn text_only(message: &Message) -> bool { // 流式增量只含文本/思考块，需要与同 id 旧消息拼接
    !message.content.is_empty() && message.content.iter().all(|block| matches!(block, MessageContent::Text(_) | MessageContent::Thinking(_))) // 非空且全部为文本或思考块
}
fn merge_delta(existing: &mut Message, incoming: &Message) { // 把同 id 的流式增量拼到旧消息
    for block in incoming.content.iter() { // 遍历增量内容块
        match block { // 按内容块类型合并
            MessageContent::Text(item) => { if let Some(MessageContent::Text(last)) = existing.content.last_mut() { last.text.push_str(&item.text); } else { existing.content.push(block.clone()); } } // 文本块拼到同类型尾部
            MessageContent::Thinking(item) => { if let Some(MessageContent::Thinking(last)) = existing.content.last_mut() { last.thinking.push_str(&item.thinking); } else { existing.content.push(block.clone()); } } // 思考块拼到同类型尾部
            _ => { existing.content.push(block.clone()); } // 其他块直接追加
        }
    }
}
pub fn append_merged(conn: &Connection, session_id: &str, message: &Message) -> rusqlite::Result<i64> { // 追加消息：文本增量合并，否则直接 upsert
    if text_only(message) { // 仅文本消息尝试增量合并
        if let Some(id) = message.id.as_deref() { // 必须有消息 id 才能合并
            if let Some(mut existing) = messages::list(conn, session_id)?.into_iter().find(|item| item.id.as_deref() == Some(id)) { // 按消息 id 查找同 id 旧消息
                merge_delta(&mut existing, message); // 合并增量到旧消息
                messages::update(conn, id, &existing)?; // 合并后更新原消息
                return conn.query_row("SELECT seq FROM messages WHERE id = ?", [id], |row| row.get(0)); // 查询更新后的自增序号
            }
        }
    }
    messages::upsert(conn, session_id, message) // 无同 id 旧消息时直接 upsert
}
pub fn append(conn: &Connection, session_id: &str, message: &Message) -> rusqlite::Result<i64> { // 直接 upsert 消息
    messages::upsert(conn, session_id, message) // 写入或替换消息
}
pub fn usage_json(usage: &ProviderUsage) -> Value { // 将模型用量转成 JSON 记账
    let input = usage.usage.input_tokens.unwrap_or(0) as i64; // 输入 token
    let output = usage.usage.output_tokens.unwrap_or(0) as i64; // 输出 token
    let cache_read = usage.usage.cache_read_input_tokens.unwrap_or(0) as i64; // 缓存读取 token
    let cache_write = usage.usage.cache_write_input_tokens.unwrap_or(0) as i64; // 缓存写入 token
    let total = usage.usage.total_tokens.map(|value| value as i64).unwrap_or(input + output); // 总量缺失时按输入输出求和
    json!({"input": input, "output": output, "cache_read": cache_read, "cache_write": cache_write, "reasoning": 0, "total": total}) // 组装用量 JSON
}
pub fn responses(conversation: &Conversation) -> HashMap<String, Permission> { // 收集对话中已回答的确认请求
    let mut map = HashMap::new(); // id 到权限的映射
    for message in conversation.iter() { // 遍历所有消息
        for content in message.content.iter() { // 遍历消息内容块
            if let MessageContent::ActionRequired(action) = content { // 只关注审批内容块
                if let ActionRequiredData::ToolConfirmationResponse { id, permission } = &action.data { map.insert(id.clone(), permission.clone()); } // 记录确认 id 到权限映射
            }
        }
    }
    map // 返回已回答映射
}
pub fn pending_actions(conversation: &Conversation) -> Vec<Value> { // 收集尚未回答的确认请求
    let answered = responses(conversation); // 已回答的请求映射
    let mut seen: Vec<String> = Vec::new(); // 去重集合
    let mut items: Vec<Value> = Vec::new(); // 待确认项缓冲
    for message in conversation.iter() { // 遍历所有消息
        for content in message.content.iter() { // 遍历消息内容块
            if let MessageContent::ActionRequired(action) = content { // 只关注审批内容块
                if let ActionRequiredData::ToolConfirmation { id, tool_name, arguments, prompt } = &action.data { // 匹配工具确认请求
                    if answered.contains_key(id) || seen.contains(id) { continue; } // 已回答或已出现过的请求跳过
                    seen.push(id.clone()); // 标记已出现
                    items.push(json!({"id": id, "toolName": tool_name, "arguments": arguments, "prompt": prompt})); // 记录待确认项
                }
            }
        }
    }
    items // 返回待确认列表
}
pub fn has_pending(conversation: &Conversation) -> bool { // 是否存在待用户确认的请求
    !pending_actions(conversation).is_empty() // 待确认列表非空即为 true
}
pub fn confirmation_message(id: &str, permission: Permission) -> Message { // 构造用户侧确认响应消息（不可见）
    Message::user().with_visibility(false, false).with_content(MessageContent::action_required_tool_confirmation_response(id, permission)) // 不可见消息携带确认结果
}
pub fn last_assistant_text(conversation: &Conversation) -> String { // 取最后一条助手文本
    conversation.messages().iter().rev().find(|message| message.role == rmcp::model::Role::Assistant).map(|message| message.as_concat_text().trim().to_string()).unwrap_or_default() // 倒序查找助手消息并取文本
}
fn mutate(conn: &Connection, session_id: &str, message_id: &str, action: impl FnOnce(&mut Message)) -> rusqlite::Result<()> { // 通用更新：按 id 找到消息后执行修改
    let all = messages::list(conn, session_id)?; // 读取会话全部消息
    if let Some(mut target) = all.into_iter().find(|item| item.id.as_deref() == Some(message_id)) { // 定位目标消息
        action(&mut target); // 执行修改闭包
        messages::update(conn, message_id, &target)?; // 更新写回数据库
    }
    Ok(()) // 未找到消息则不做任何改动
}
fn set_note(conn: &Connection, session_id: &str, message_id: &str, operation: &str, key: &str, value: &Value) -> rusqlite::Result<()> { // 写入消息操作备注
    mutate(conn, session_id, message_id, |message| message.metadata.set_operation_note(operation, key, value.clone())) // 委托通用更新设置备注
}
fn set_visibility(conn: &Connection, session_id: &str, message_id: &str, user_visible: bool, agent_visible: bool) -> rusqlite::Result<()> { // 更新消息可见性
    mutate(conn, session_id, message_id, |message| { message.metadata.user_visible = user_visible; message.metadata.agent_visible = agent_visible; }) // 委托通用更新设置可见性
}
fn patch_tool_request(conn: &Connection, session_id: &str, tool_call_id: &str, patch: &Value) -> rusqlite::Result<()> { // 按工具调用 id 打补丁
    for mut message in messages::list(conn, session_id)? { // 遍历会话全部消息
        let Some(id) = message.id.clone() else { continue; }; // 无 id 的消息跳过
        let mut hit = false; // 是否命中目标请求
        for content in message.content.iter_mut() { // 遍历消息内容块
            if let MessageContent::ToolRequest(request) = content { // 定位 ToolRequest 内容块
                if request.id == tool_call_id { // 仅匹配目标 tool_call_id
                    let meta = request.tool_meta.get_or_insert_with(|| json!({})); // 元数据不存在则初始化
                    if let (Some(target), Some(extra)) = (meta.as_object_mut(), patch.as_object()) { for (key, value) in extra { target.insert(key.clone(), value.clone()); } } // 合并补丁键值到元数据
                    hit = true; // 标记命中
                }
            }
        }
        if hit { messages::update(conn, &id, &message)?; } // 命中后写回数据库
    }
    Ok(()) // 补丁完成
}
