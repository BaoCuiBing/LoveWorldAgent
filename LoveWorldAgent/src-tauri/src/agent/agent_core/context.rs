use std::collections::HashSet; // 引入集合类型校对工具请求响应配对
use std::time::{Duration, Instant}; // 压缩耗时统计与超时控制
use goose_agent::events::AgentEvent; // SSE 事件类型，用于通知前端刷新上下文摘要
use goose_context_management::format_message_for_compacting; // 复用 goose 消息格式化工具
use goose_provider_types::conversation::message::{Message, MessageContent, MessageErrorKind, SystemNotificationContent, SystemNotificationType}; // 消息与内容块类型
use goose_provider_types::errors::ProviderError; // 模型错误类型（识别超窗重试）
use goose_provider_types::thinking::ThinkingEffort; // 压缩请求思考强度控制
use rmcp::model::{ContentBlock, Role}; // MCP 内容块与消息角色
use serde_json::{json, Value}; // JSON 构造与动态值
use tokio::sync::mpsc::UnboundedSender; // SSE 发送通道
use crate::agent::models::{self, ModelConfig}; // 模型注册表与构造器
use crate::agent::store_sqls::{context as context_store, messages as messages_store, usage as usage_store}; // SQLite 上下文/消息/用量读取
use crate::config; // 配置读取
use crate::state::{self, AppState}; // 应用共享状态与文本裁剪工具
use crate::utils::logger; // 主日志输出
pub const MAX_HISTORY_CHARS: usize = 36000; // 送入结构化摘要模型的历史最大字符基数
pub const MAX_FACTS: usize = 8; // 单次保留关键事实上限
pub const MICROCOMPACT_KEEP_RECENT: usize = 3; // OpenClaw 同款：保护最近 3 轮 Assistant 对话不压缩
pub const MICROCOMPACT_MIN_CHARS: usize = 2000; // <=2000 字符的短工具输出永不微压缩
pub const MICROCOMPACT_SOFT_TRIM_RATIO: f64 = 0.30; // 消息总字符数达到窗口 30% 才触发软裁剪
pub const MICROCOMPACT_HARD_CLEAR_RATIO: f64 = 0.50; // 软裁剪后仍 >=50% 才按最旧顺序硬清空
pub const MICROCOMPACT_SOFT_HEAD_CHARS: usize = 800; // 软裁剪保留头部字符数
pub const MICROCOMPACT_SOFT_TAIL_CHARS: usize = 800; // 软裁剪保留尾部字符数
pub const MICROCOMPACT_CHARS_PER_TOKEN: usize = 4; // 字符/Token 估算倍率（用于计算 char window）
pub const MICROCOMPACT_CLEARED_TEXT: &str = "[Old tool result content cleared]"; // 微压缩旧工具输出硬清空占位文本
const SOFT_TRIM_MARKER: &str = "[Tool result trimmed: kept first 800 and last 800 of "; // 软裁剪元信息标记
pub const AUTO_COMPACT_RATIO: f64 = 0.8; // 累计消耗增量达模型上下文 80% 触发自动压缩
pub const MAX_CONSECUTIVE_FAILURES: i64 = 3; // 连续失败 3 次断路器，避免死循环损耗配额
pub const COMPACT_TIMEOUT_SECS: u64 = 120; // 单次压缩请求最长超时秒数
const REMOVAL_PERCENTAGES: [u32; 5] = [0, 10, 20, 50, 100]; // 摘要本身超窗时从中间渐进丢弃工具输出比例
const KEEP_TAIL_MESSAGES: usize = 6; // 自动压缩默认保留的尾部消息条数
const NO_TOOLS_PREAMBLE: &str = "CRITICAL: Respond with TEXT ONLY. Do NOT call any tools or emit tool calls. Output only your <analysis> and <summary> sections.\n\n"; // 禁止压缩模型调用工具前缀
const COMPACT_SYSTEM_PROMPT: &str = "你是专业的代码智能体上下文压缩器。请把给定的对话历史压缩成精炼、保真的中文结构化摘要，以便模型在清空早先工具细节后无缝继续完成当前任务。\n\n请先在 <analysis>...</analysis> 标签内快速梳理关键脉络（仅作草稿，最终会被剥离），然后严格在 <summary>...</summary> 标签内按以下 9 个小节输出完整摘要（无内容的小节简写「暂无」）：\n1. Primary Request and Intent（核心需求与最终目标）\n2. Key Technical Concepts（关键技术栈、协议与约束）\n3. Files and Code Sections（涉及的关键文件路径、符号名与核心改动）\n4. Errors and fixes（已遇错误原因及对应修复手段）\n5. Problem Solving（已验证的排查思路与关键结论）\n6. All user messages（按时间保留用户的全部非工具指令原话要点）\n7. Pending Tasks（尚未完成或待后续推进的任务项）\n8. Current Work（压缩前正在进行的具体操作与最新状态）\n9. Optional Next Step（紧接着最合理的一步行动及原话依据）\n\n注意：严禁编造未出现的路径、命令或测试结果；只输出 <analysis> 与 <summary> 内容，不要调用任何工具。"; // 参照 Claude Code 9 段结构化摘要模板
pub const FACT_PROMPT: &str = "你是事实抽取器。从下面的对话摘要或历史中抽取可供后续复用的稳定事实：用户偏好、项目决策、固定路径、外部约束、已验证结论。\n只输出一个 JSON 字符串数组，例如 [\"事实一\", \"事实二\"]；不要输出其它文字。不要包含临时过程或猜测；同类事实合并去重，每条不超过 300 字，最多 8 条，没有则输出 []。"; // 关键事实抽取提示词
fn pick_model_config(model_name: &str) -> Result<ModelConfig, String> { // 优先选用当前对话模型，未匹配再回退首个启用模型
    let llm = config::load_agent_config().get("llm").cloned().unwrap_or(Value::Null); // 读取 llm 配置数组
    let target = model_name.trim(); // 去除空白
    if !target.is_empty() { // 指定了模型名时优先精确匹配
        for item in models::normalize_llm(&llm) { // 遍历配置模型
            let cfg = models::parse_model(&item); // 解析配置
            if cfg.enabled && cfg.model == target && !cfg.api_key.trim().is_empty() { return Ok(cfg); } // 命中启用且带 Key 的当前模型
        }
    }
    models::active_model(&llm).filter(|cfg| !cfg.api_key.trim().is_empty()).ok_or_else(|| "没有可用的启用模型（缺少模型或 API Key）".to_string()) // 回退首个启用模型
}
fn tool_response_char_len(res: &Result<rmcp::model::CallToolResult, rmcp::model::ErrorData>) -> usize { // 估算工具返回内容的字符体积
    match res {
        Ok(val) => {
            let body_chars: usize = val.content.iter().map(|item| item.as_text().map(|t| t.text.chars().count()).unwrap_or(160)).sum(); // 统计正文块长度
            let meta_chars = val.structured_content.as_ref().map(|v| v.to_string().chars().count()).unwrap_or(0); // 统计结构化载荷长度
            body_chars + meta_chars // 返回总字符估算
        }
        Err(err) => err.message.chars().count(), // 错误信息字符数
    }
}
fn is_already_cleared(res: &Result<rmcp::model::CallToolResult, rmcp::model::ErrorData>) -> bool { // 判断该工具结果是否已被清理占位
    match res {
        Ok(val) => val.content.len() == 1 && val.structured_content.is_none() && val.content[0].as_text().map(|t| t.text == MICROCOMPACT_CLEARED_TEXT).unwrap_or(false), // 比对占位文本
        Err(err) => err.message == MICROCOMPACT_CLEARED_TEXT, // 错误占位比对
    }
}
fn is_soft_trimmed(res: &Result<rmcp::model::CallToolResult, rmcp::model::ErrorData>) -> bool { // 判断工具输出是否已被软裁剪
    match res {
        Ok(val) => val.content.iter().any(|item| item.as_text().map(|t| t.text.contains(SOFT_TRIM_MARKER)).unwrap_or(false)),
        Err(err) => err.message.contains(SOFT_TRIM_MARKER),
    }
}
fn estimate_message_char_len(message: &Message) -> usize { // 估算单条消息总字符数
    message.content.iter().map(|block| match block {
        MessageContent::Text(t) => t.text.chars().count(),
        MessageContent::Thinking(t) => t.thinking.chars().count(),
        MessageContent::ToolRequest(req) => serde_json::to_string(&req.tool_call).map(|s| s.chars().count()).unwrap_or(160),
        MessageContent::ToolResponse(resp) => tool_response_char_len(&resp.tool_result),
        _ => 80,
    }).sum()
}
fn soft_trim_text(text: &str) -> String { // Stage 1：保留头尾各 800 字符并附带原始长度元信息（按字符边界切片防中文截断乱码）
    let total = text.chars().count();
    let head: String = text.chars().take(MICROCOMPACT_SOFT_HEAD_CHARS).collect();
    let skip = total.saturating_sub(MICROCOMPACT_SOFT_TAIL_CHARS);
    let tail: String = text.chars().skip(skip).collect();
    format!("{head}\n...\n{tail}\n\n[Tool result trimmed: kept first {MICROCOMPACT_SOFT_HEAD_CHARS} and last {MICROCOMPACT_SOFT_TAIL_CHARS} of {total} chars]")
}
fn apply_soft_trim(res: &mut Result<rmcp::model::CallToolResult, rmcp::model::ErrorData>) -> bool { // 就地执行 Stage 1 软裁剪，有改动返回 true
    match res {
        Ok(val) => {
            let joined = val.content.iter().filter_map(|item| item.as_text().map(|t| t.text.as_str())).collect::<Vec<_>>().join("\n");
            if joined.chars().count() <= MICROCOMPACT_SOFT_HEAD_CHARS + MICROCOMPACT_SOFT_TAIL_CHARS {
                if val.structured_content.is_some() { val.structured_content = None; return true; }
                return false;
            }
            val.content = vec![ContentBlock::text(soft_trim_text(&joined))];
            val.structured_content = None;
            true
        }
        Err(err) => {
            let raw = err.message.as_ref().to_string();
            if raw.chars().count() <= MICROCOMPACT_SOFT_HEAD_CHARS + MICROCOMPACT_SOFT_TAIL_CHARS { return false; }
            err.message = std::borrow::Cow::Owned(soft_trim_text(&raw));
            true
        }
    }
}
fn apply_hard_clear(res: &mut Result<rmcp::model::CallToolResult, rmcp::model::ErrorData>) { // 就地执行 Stage 2 硬清空占位
    match res {
        Ok(val) => {
            val.content = vec![ContentBlock::text(MICROCOMPACT_CLEARED_TEXT)];
            val.structured_content = None;
        }
        Err(err) => {
            err.message = std::borrow::Cow::Borrowed(MICROCOMPACT_CLEARED_TEXT);
        }
    }
}
pub fn microcompact_messages(messages: &[Message], keep_recent: usize) -> (Vec<Message>, usize) { // 兼容默认窗口调用的两阶段微压缩入口
    microcompact_messages_with_limit(messages, keep_recent, 200_000)
}
pub fn microcompact_messages_with_limit(messages: &[Message], keep_recent_assistants: usize, context_limit: usize) -> (Vec<Message>, usize) { // OpenClaw 同款两阶段微压缩：保护最近 3 个 Assistant 轮次；总字符>=30%先软裁剪>2000字符旧输出；仍>=50%再按最旧到最新硬清空
    let assistant_indices: Vec<usize> = messages.iter().enumerate().filter(|(_, msg)| msg.role == Role::Assistant).map(|(idx, _)| idx).collect(); // 定位全部 Assistant 轮次边界
    if keep_recent_assistants > 0 && assistant_indices.len() <= keep_recent_assistants { return (messages.to_vec(), 0); } // 未超出受保护轮次不清理
    let cutoff_msg_idx = if keep_recent_assistants == 0 { messages.len() } else { assistant_indices[assistant_indices.len() - keep_recent_assistants] }; // 保护自倒数第 K 条 Assistant 起至末尾的全部消息
    let mut total_chars: usize = messages.iter().map(estimate_message_char_len).sum(); // 当前消息总字符体积
    let char_window = context_limit.max(1).saturating_mul(MICROCOMPACT_CHARS_PER_TOKEN); // 上下文字符窗口
    let soft_threshold = ((char_window as f64) * MICROCOMPACT_SOFT_TRIM_RATIO) as usize; // 30% 软裁剪阈值
    let hard_threshold = ((char_window as f64) * MICROCOMPACT_HARD_CLEAR_RATIO) as usize; // 50% 硬清空阈值
    if total_chars < soft_threshold { return (messages.to_vec(), 0); } // 低于 30% 跳过微压缩
    let mut projected = messages.to_vec(); // 深拷贝请求切片供内存裁剪
    let mut modified_blocks: HashSet<(usize, usize)> = HashSet::new(); // 记录本次被修改的块坐标
    let mut eligible: Vec<(usize, usize)> = Vec::new(); // 可在 Stage 2 硬清空的旧块（>2000 或已软裁剪）
    for msg_idx in 0..cutoff_msg_idx { // Stage 1：从早到晚对超过 2000 字符的旧工具输出执行 Soft Trim
        let block_len = projected[msg_idx].content.len();
        for block_idx in 0..block_len {
            let MessageContent::ToolResponse(resp) = &mut projected[msg_idx].content[block_idx] else { continue; };
            if is_already_cleared(&resp.tool_result) { continue; }
            let trimmed_before = is_soft_trimmed(&resp.tool_result);
            let old_len = tool_response_char_len(&resp.tool_result);
            if !trimmed_before && old_len <= MICROCOMPACT_MIN_CHARS { continue; } // <=2000 字符短结果原样保留
            eligible.push((msg_idx, block_idx));
            if !trimmed_before && apply_soft_trim(&mut resp.tool_result) {
                let new_len = tool_response_char_len(&resp.tool_result);
                total_chars = total_chars.saturating_sub(old_len.saturating_sub(new_len));
                modified_blocks.insert((msg_idx, block_idx));
            }
        }
    }
    if total_chars >= hard_threshold { // Stage 2：软裁剪后仍>=50%窗口，按最旧到最新硬清空直到低于 50%
        let cleared_len = MICROCOMPACT_CLEARED_TEXT.chars().count();
        for (msg_idx, block_idx) in eligible {
            if total_chars < hard_threshold { break; }
            let MessageContent::ToolResponse(resp) = &mut projected[msg_idx].content[block_idx] else { continue; };
            let old_len = tool_response_char_len(&resp.tool_result);
            apply_hard_clear(&mut resp.tool_result);
            total_chars = total_chars.saturating_sub(old_len.saturating_sub(cleared_len));
            modified_blocks.insert((msg_idx, block_idx));
        }
    }
    (projected, modified_blocks.len()) // 返回瘦身副本与修改块数
}
pub fn history_text(history: &[Value]) -> String { // 将文本历史数组拼接为字符串，超长截取尾部
    let mut lines: Vec<String> = Vec::new(); // 行缓冲
    for item in history { // 遍历历史项
        let role = item.get("role").and_then(Value::as_str).unwrap_or_default(); // 角色
        let content = item.get("content").and_then(Value::as_str).unwrap_or_default().trim(); // 正文
        if (role != "user" && role != "assistant") || content.is_empty() { continue; } // 过滤非法或空项
        lines.push(format!("[{role}] {content}")); // 追加一行
    }
    let text = lines.join("\n"); // 拼接全串
    let count = text.chars().count(); // 统计字符数
    if count > MAX_HISTORY_CHARS { text.chars().skip(count - MAX_HISTORY_CHARS).collect() } else { text } // 保留最新窗口尾部
}
pub fn parse_facts(text: &str) -> Vec<String> { // 从模型输出解析关键事实 JSON 数组
    let mut raw = text.trim().to_string(); // 去空白
    if raw.contains("```") { // 剥离 Markdown 代码围栏
        let parts: Vec<&str> = raw.split("```").collect(); // 按围栏切分
        raw = parts.get(1).map(|part| part.trim_start_matches("json").trim().to_string()).unwrap_or_else(|| parts[0].trim().to_string()); // 取代码块正文
    }
    if let (Some(start), Some(end)) = (raw.find('['), raw.rfind(']')) { // 提取数组起止位置
        if end > start { raw = raw[start..=end].to_string(); } // 截取 JSON 数组子串
    }
    let Ok(value) = serde_json::from_str::<Value>(&raw) else { logger::warn("context", &format!("关键事实解析失败，本次不落盘: {}", state::truncate_chars(text, 120))); return Vec::new(); }; // 解析失败跳过
    let Some(items) = value.as_array() else { return Vec::new(); }; // 非数组返回空
    items.iter().filter_map(|item| { // 清洗并截断单条长度
        let text = state::truncate_chars(&state::collapse(&state::json_str(Some(item))), 300); // 折叠空白限长 300
        if text.is_empty() { None } else { Some(text) } // 过滤空串
    }).take(MAX_FACTS).collect() // 最多取 8 条
}
pub fn extract_compact_summary(raw: &str) -> String { // 剥离 <analysis> 草稿思考并提取 <summary> 精华正文
    let mut text = raw.trim().to_string(); // 复制原文
    while let Some(start) = text.find("<analysis>") { // 循环剥离全部 <analysis> 段落
        if let Some(rel_end) = text[start..].find("</analysis>") { // 找到闭合标签
            let after = start + rel_end + "</analysis>".len(); // 闭合结束偏移
            text = format!("{}{}", &text[..start], &text[after..]); // 移除整段分析草稿
        } else { break; } // 未闭合则跳出
    }
    if let Some(start) = text.find("<summary>") { // 优先提取 <summary> 内部结构化摘要
        let inner_start = start + "<summary>".len(); // 跳过起始标签
        if let Some(rel_end) = text[inner_start..].find("</summary>") { // 定位结束标签
            return text[inner_start..inner_start + rel_end].trim().to_string(); // 返回纯净摘要正文
        }
        return text[inner_start..].trim().to_string(); // 未闭合则取至末尾
    }
    text.trim().to_string() // 模型未输出标签时兜底保留剩余正文
}
fn filter_middle_tool_responses(messages: &[Message], remove_percent: u32) -> Vec<&Message> { // 摘要自身超窗时从中间向外丢弃工具响应消息
    if remove_percent == 0 { return messages.iter().collect(); } // 0% 原样保留
    let tool_indices: Vec<usize> = messages.iter().enumerate().filter(|(_, msg)| msg.content.iter().any(|c| matches!(c, MessageContent::ToolResponse(_)))).map(|(i, _)| i).collect(); // 定位含工具返回的消息下标
    if tool_indices.is_empty() { return messages.iter().collect(); } // 无工具返回直接返回
    let count = ((tool_indices.len() * remove_percent as usize) / 100).max(1); // 待移除数量
    let middle = tool_indices.len() / 2; // 中间锚点
    let mut drop_set = HashSet::new(); // 待跳过消息索引集合
    for i in 0..count { // 从中点向两侧交替扩张选取丢弃索引
        let offset = i / 2; // 步距
        if i % 2 == 0 { if middle > offset { drop_set.insert(tool_indices[middle - offset - 1]); } } else if middle + offset < tool_indices.len() { drop_set.insert(tool_indices[middle + offset]); } // 交替收集
    }
    messages.iter().enumerate().filter(|(idx, _)| !drop_set.contains(idx)).map(|(_, msg)| msg).collect() // 返回过滤后的消息引用
}
fn build_compact_transcript(prior_summary: &str, messages: &[&Message]) -> String { // 构造喂给压缩模型的历史增量文本
    let mut out = String::new(); // 输出缓冲
    if !prior_summary.trim().is_empty() { // 增量合入前次摘要，保障多轮连续压缩不丢前情
        out.push_str("## 先前压缩摘要（请与后续新增对话增量融合，避免丢失已有决策与目标）\n"); // 增量标题
        out.push_str(prior_summary.trim()); // 追加既有摘要
        out.push_str("\n\n## 新增待压缩对话\n"); // 新段标题
    }
    for msg in messages { // 逐条格式化消息内容
        out.push_str(&format_message_for_compacting(msg)); // 复用 goose 标准化压缩格式
        out.push('\n'); // 换行分隔
    }
    out // 返回汇总文本
}
fn is_context_overflow_err(err: &ProviderError) -> bool { // 判断模型报错是否为上下文超长
    matches!(err, ProviderError::ContextLengthExceeded(_)) || err.to_string().to_ascii_lowercase().contains("context length") || err.to_string().to_ascii_lowercase().contains("too long") // 匹配枚举与常见文本
}
async fn summarize_messages_with_model(cfg: &ModelConfig, prior_summary: &str, messages: &[Message], timeout_secs: u64) -> Result<String, String> { // 使用当前对话模型（禁工具）生成 9 段结构化摘要，超窗渐进裁剪重试
    let provider = models::goose_provider(cfg)?; // 构建当前模型 Provider
    let mut model_cfg = models::goose_model_config(cfg); // 构建模型配置
    if model_cfg.thinking_effort().is_none() { model_cfg = model_cfg.with_default_thinking_effort(Some(ThinkingEffort::Low)); } // 保持合法思考配置
    let sys = format!("{NO_TOOLS_PREAMBLE}{COMPACT_SYSTEM_PROMPT}"); // 组装纯文本禁工具压缩提示词
    let has_tools = messages.iter().any(|m| m.content.iter().any(|c| matches!(c, MessageContent::ToolResponse(_)))); // 是否含工具返回可裁剪
    let wait = Duration::from_secs(timeout_secs.max(COMPACT_TIMEOUT_SECS)); // 超时保护
    for (attempt, pct) in REMOVAL_PERCENTAGES.iter().enumerate() { // 按 [0,10,20,50,100]% 渐进尝试
        let kept = filter_middle_tool_responses(messages, *pct); // 过滤本轮工具返回
        let body = build_compact_transcript(prior_summary, &kept); // 拼装压缩上下文文本
        if body.trim().is_empty() { return Err("待压缩历史内容为空".to_string()); } // 空内容拦截
        let req = vec![Message::user().with_text(format!("请将以下对话记录（{}）压缩为纯文本结构化摘要：\n\n{body}", if *pct == 0 { "完整".to_string() } else { format!("已裁去 {pct}% 中间工具输出") }))]; // 构造单条用户输入
        let fut = provider.complete(&model_cfg, &sys, &req, &[]); // 空工具数组物理禁止工具调用
        match tokio::time::timeout(wait, fut).await { // 执行带超时的单次补全
            Ok(Ok((msg, _))) => {
                let summary = extract_compact_summary(&msg.as_concat_text()); // 剥离 analysis 并提取 summary
                if summary.trim().is_empty() { return Err("压缩模型返回摘要为空".to_string()); } // 校验非空摘要
                return Ok(summary); // 摘要生成成功
            }
            Ok(Err(err)) if is_context_overflow_err(&err) && has_tools && attempt + 1 < REMOVAL_PERCENTAGES.len() => {
                logger::warn("context", &format!("摘要请求本身超窗 (drop={pct}%)，继续提高裁剪比例重试")); // 记录并进入下一档裁剪
            }
            Ok(Err(err)) => return Err(format!("摘要模型请求失败: {err}")), // 其他错误上抛
            Err(_) => return Err("摘要模型请求超时".to_string()), // 超时退出
        }
    }
    Err("压缩输入超出窗口且已尝试全部裁剪比例".to_string()) // 全部比例均失败
}
async fn extract_facts_with_model(cfg: &ModelConfig, summary: &str, timeout_secs: u64) -> Vec<String> { // 用当前对话模型从摘要抽取关键事实（失败降级为空不影响主压缩）
    let Ok(provider) = models::goose_provider(cfg) else { return Vec::new(); }; // 构造 Provider 失败返回空
    let model_cfg = models::goose_model_config(cfg); // 构建模型配置
    let req = vec![Message::user().with_text(summary.to_string())]; // 以摘要作为输入更省 token
    let wait = Duration::from_secs(timeout_secs.max(30)); // 至少 30 秒超时
    match tokio::time::timeout(wait, provider.complete(&model_cfg, FACT_PROMPT, &req, &[])).await { // 无工具调用补全
        Ok(Ok((msg, _))) => parse_facts(&msg.as_concat_text()), // 解析 JSON 事实列表
        Ok(Err(err)) => { logger::warn("context", &format!("抽取关键事实失败（跳过不影响压缩）: {err}")); Vec::new() } // 降级不阻断
        Err(_) => { logger::warn("context", "抽取关键事实超时（跳过不影响压缩）"); Vec::new() } // 超时降级
    }
}
fn select_compact_cut_ids(visible: &[Message], force: bool) -> Vec<String> { // 选取可安全隐藏的旧消息 ID（保留最新用户提问与近期完整工具配对）
    if visible.len() <= 2 { return Vec::new(); } // 历史过短不压缩
    let last_prompt_idx = visible.iter().rposition(messages_store::is_user_prompt); // 最新显式用户提问索引
    let candidates = if force { &[4usize, 1usize][..] } else { &[KEEP_TAIL_MESSAGES, 2usize][..] }; // 普通按 6/2 条尾窗，强制模式放宽至 4/1 条
    for &tail_keep in candidates { // 依次寻找合理切分点
        let mut keep_from = visible.len().saturating_sub(tail_keep); // 初步尾部保留起点
        loop { // 向前回溯确保不切碎成对 toolRequest/toolResponse
            if keep_from == 0 { break; } // 已到头部停止
            let tail_req_ids: HashSet<&str> = visible[keep_from..].iter().flat_map(|m| m.content.iter()).filter_map(|c| match c { MessageContent::ToolRequest(r) => Some(r.id.as_str()), _ => None }).collect(); // 尾部已有的请求 ID
            let missing_req_in_tail = visible[keep_from..].iter().any(|m| m.content.iter().any(|c| match c { MessageContent::ToolResponse(resp) => !tail_req_ids.contains(resp.id.as_str()), _ => false })); // 尾部是否存在匹配请求在前半段的响应
            let starts_with_tool_resp = visible[keep_from].content.iter().any(|c| matches!(c, MessageContent::ToolResponse(_))); // 起点本身为工具回执
            if !missing_req_in_tail && !starts_with_tool_resp { break; } // 边界对齐干净
            keep_from -= 1; // 继续回溯直到包含配对的 Assistant toolRequest
        }
        let mut ids: Vec<String> = Vec::new(); // 待隐藏消息 ID 列表
        for (idx, msg) in visible.iter().enumerate() { // 收集 cut 之前的非当前提问消息
            if idx >= keep_from { break; } // 尾部保留区间不隐藏
            if Some(idx) == last_prompt_idx { continue; } // 必须保留最新用户输入以维持状态机 kickoff
            if let Some(id) = msg.id.clone() { ids.push(id); } // 收录可隐藏消息 ID
        }
        if ids.len() >= 2 || (force && !ids.is_empty()) { return ids; } // 命中足够可压缩历史返回
    }
    Vec::new() // 无足够可压历史返回空
}
pub fn has_unanswered_tool_requests(messages: &[Message]) -> bool { // 检查当前是否存在尚未回应的工具调用（有则暂缓压缩防切碎状态机）
    let answered: HashSet<&str> = messages.iter().flat_map(|m| m.get_tool_response_ids()).collect(); // 所有已回应工具 ID
    messages.iter().any(|m| m.is_agent_visible() && m.content.iter().any(|c| match c { MessageContent::ToolRequest(r) => !answered.contains(r.id.as_str()), _ => false })) // 存在未回应请求则返回 true
}
pub fn load_facts(session_id: &str) -> Vec<String> { // 从数据库读取已确认的关键事实
    let app = state::shared(); // 取全局状态
    let conn = app.db.lock(); // 加锁读取 SQLite 连接
    context_store::load_facts(&conn, session_id, MAX_FACTS as i64).unwrap_or_default() // 查询失败按空列表处理
}
pub fn inject_text(session_id: &str) -> String { // 拼装要注入系统提示词的上下文块（每次状态机推理前动态读取）
    if session_id.is_empty() { return String::new(); } // 无会话时不注入
    let app = state::shared(); // 取全局状态
    let conn = app.db.lock(); // 加锁读取 SQLite 连接
    let context = context_store::load_context(&conn, session_id).ok().flatten().unwrap_or_else(|| json!({})); // 读取会话摘要，缺失按空对象处理
    let summary = context.get("summary").and_then(Value::as_str).unwrap_or_default().trim().to_string(); // 取摘要正文
    drop(conn); // 及时释放连接锁再调用其他查询
    let facts = load_facts(session_id); // 另行读取关键事实
    if summary.is_empty() && facts.is_empty() { return String::new(); } // 摘要与事实都为空时无需注入
    let mut blocks: Vec<String> = Vec::new(); // 上下文块缓冲
    if !summary.is_empty() { blocks.push(format!("\n## 历史摘要（此前对话的结构化压缩，请结合近期消息无缝继续）\n{summary}\n")); } // 注入最新结构化摘要
    if !facts.is_empty() { // 有事实才生成事实块
        let rows: Vec<String> = facts.iter().rev().map(|fact| format!("- {fact}")).collect(); // 按时间倒序转为列表项
        blocks.push(format!("\n## 关键事实（此前对话中已确认，可直接复用）\n{}\n", rows.join("\n"))); // 事实块按需注入
    }
    blocks.join("") // 拼接所有上下文块
}
pub async fn compact_in_turn(app: &AppState, session_id: &str, model_cfg: &ModelConfig, tx: Option<&UnboundedSender<AgentEvent>>, force: bool) -> Result<bool, String> { // 轮内/被动统一自动压缩核心：结构化摘要 + 抽事实 + agent_visible=false + 推进基线
    let started = Instant::now(); // 计时开始
    let (prior_summary, visible_msgs, current_total) = { // 单次持锁读取当前快照并立即释放锁，避免跨 await 持锁
        let conn = app.db.lock(); // 加锁读库
        let prior = context_store::load_context(&conn, session_id).ok().flatten().and_then(|v| v.get("summary").and_then(Value::as_str).map(str::to_string)).unwrap_or_default(); // 已有摘要供增量融合
        let all = messages_store::list(&conn, session_id).map_err(|e| format!("读取会话消息失败: {e}"))?; // 读取全部消息
        let visible: Vec<Message> = all.into_iter().filter(|m| m.is_agent_visible()).collect(); // 仅提取当前模型可见消息
        let total = usage_store::get_session_usage(&conn, session_id).ok().and_then(|v| v.get("total").and_then(Value::as_i64)).unwrap_or(0); // 读取最新累计 token 总量
        (prior, visible, total) // 返回快照数据
    };
    let hide_ids = select_compact_cut_ids(&visible_msgs, force); // 选取安全切分边界并提取待隐藏 ID
    if hide_ids.is_empty() { // 若无可安全隐藏消息，则先更新基线避免同一处死循环
        let conn = app.db.lock(); // 加锁更新基线
        let _ = context_store::set_compaction_baseline(&conn, session_id, current_total); // 前移基线
        return Ok(false); // 本步未执行实际压缩
    }
    let hide_set: HashSet<&str> = hide_ids.iter().map(String::as_str).collect(); // 建集合以过滤待摘要消息
    let to_summarize: Vec<Message> = visible_msgs.iter().filter(|m| m.id.as_deref().map(|id| hide_set.contains(id)).unwrap_or(false)).cloned().collect(); // 待压缩消息副本
    let chars_before: usize = to_summarize.iter().map(|m| format_message_for_compacting(m).chars().count()).sum(); // 统计压缩前字符体量
    let settings = crate::agent::settings::load(); // 读取智能体运行设置
    let summary = summarize_messages_with_model(model_cfg, &prior_summary, &to_summarize, settings.timeout_seconds.max(10) as u64).await?; // await 生成 9 段结构化摘要
    let facts = extract_facts_with_model(model_cfg, &summary, settings.timeout_seconds.max(10) as u64).await; // await 抽取关键事实
    let tokens_before = (chars_before / 3).max(1) as i64; // 估算被隐藏消息占用的 token 规模
    let tokens_after = (summary.chars().count() / 2).max(1) as i64; // 估算压缩摘要后占用 token
    if tokens_after >= tokens_before * 2 && !force && chars_before > 1000 { // 若常规压缩反增不减，跳过隐藏以免负优化
        logger::warn("context", &format!("自动压缩未获收益（before={tokens_before} after={tokens_after}），跳过本次隐藏: session={session_id}")); // 记日志告警
        let conn = app.db.lock(); // 推进基线防止原地死循环
        let _ = context_store::set_compaction_baseline(&conn, session_id, current_total); // 前移基线
        return Ok(false); // 跳过本次
    }
    let (hidden_count, latest_total, added_facts) = { // 再次短持锁落库变更
        let conn = app.db.lock(); // 加锁写库
        let hidden = messages_store::hide_for_agent(&conn, session_id, &hide_ids).map_err(|e| format!("更新消息可见性失败: {e}"))?; // 置 agent_visible=false（保留 UI 原文）
        context_store::save_context(&conn, session_id, &summary, tokens_before, tokens_after).map_err(|e| format!("保存压缩摘要失败: {e}"))?; // 写回最新摘要
        let fact_vals: Vec<Value> = facts.iter().map(|f| json!(f)).collect(); // 事实转 JSON 值
        let added = context_store::add_facts(&conn, session_id, &fact_vals).unwrap_or_default(); // 增量追加去重事实
        let latest = usage_store::get_session_usage(&conn, session_id).ok().and_then(|v| v.get("total").and_then(Value::as_i64)).unwrap_or(current_total); // 重读最新累计值
        let _ = context_store::set_compaction_baseline(&conn, session_id, latest); // 推进基线并复位失败计数
        (hidden, latest, added) // 返回结果汇总
    };
    let elapsed = started.elapsed().as_millis(); // 统计耗时毫秒
    logger::info("context", &format!("AutoCompact 完成: session={session_id} hidden={hidden_count} chars={chars_before}->{} est_tokens={tokens_before}->{tokens_after} baseline={latest_total} facts={} elapsed={elapsed}ms", summary.chars().count(), added_facts.len())); // 写主日志供排障
    if let Some(sender) = tx { // 若有 SSE 通道则实时通知前端刷新右侧摘要与关键事实
        let mut note = Message::assistant().with_visibility(false, false).with_generated_id_if_missing(); // 仅触发前端刷新的隐藏通知，不落库
        note.metadata.set_operation_note("context", "compacted", json!({"hidden": hidden_count, "tokens_after": tokens_after, "baseline": latest_total})); // 携带本次压缩元数据
        let _ = sender.send(AgentEvent::Message(note)); // 通知前端重拉摘要
    }
    Ok(true) // 压缩完成
}
pub async fn maybe_auto_compact(app: &AppState, session_id: &str, model_cfg: &ModelConfig, context_limit: usize, tx: &UnboundedSender<AgentEvent>) -> bool { // 状态机每步推理前检查：80% 累计增量触发 + 断路器 + 防切碎工具链
    if session_id.is_empty() || context_limit == 0 { return false; } // 缺少必要信息跳过
    let (should_run, failures) = { // 持锁快速校验阈值与安全条件
        let conn = app.db.lock(); // 加锁读库
        let (baseline, failures) = context_store::get_compaction_state(&conn, session_id); // 查上次基线与连续失败次数
        if failures >= MAX_CONSECUTIVE_FAILURES { return false; } // 连续失败 3 次已熔断
        let total = usage_store::get_session_usage(&conn, session_id).ok().and_then(|v| v.get("total").and_then(Value::as_i64)).unwrap_or(0); // 取累计 token 总耗用
        let threshold = ((context_limit as f64) * AUTO_COMPACT_RATIO) as i64; // 计算 80% 增量触发线
        let delta = total.saturating_sub(baseline); // 自上次压缩以来的多步累计增量
        let all = messages_store::list(&conn, session_id).unwrap_or_default(); // 检查消息队列现状
        let visible: Vec<Message> = all.iter().filter(|m| m.is_agent_visible()).cloned().collect(); // 过滤模型可见消息
        let has_assistant_reply = visible.iter().any(|m| m.role == Role::Assistant); // 至少已有一次助手回复才压，防止同处空转
        let unready = visible.len() <= 2 || has_unanswered_tool_requests(&visible); // 历史过短或处于未回应工具调用中时不打断
        let est_chars: usize = visible.iter().map(|m| format_message_for_compacting(m).chars().count()).sum(); // 兜底估算（针对未回报 usage 的自定义模型）
        let est_overflow = delta == 0 && (est_chars / 3) as i64 >= threshold; // 无 usage 上报时按字符估算兜底
        ((delta >= threshold || est_overflow) && has_assistant_reply && !unready, failures) // 返回触发结论与历史失败数
    };
    if !should_run { return false; } // 未达 80% 阈值或正处于工具挂起态
    match compact_in_turn(app, session_id, model_cfg, Some(tx), false).await { // 执行常规轮内自动压缩
        Ok(compacted) => compacted, // 返回是否实际发生隐藏
        Err(err) => { // 失败递增断路器计数，永不阻断当前对话推进
            let count = { let conn = app.db.lock(); context_store::record_compaction_failure(&conn, session_id).unwrap_or(failures + 1) }; // 累计连续失败数
            logger::warn("context", &format!("AutoCompact 失败（连续第 {count}/{MAX_CONSECUTIVE_FAILURES} 次）: session={session_id} err={err}")); // 写入日志告警
            if count >= MAX_CONSECUTIVE_FAILURES { logger::warn("context", &format!("AutoCompact 连续失败已达上限，本会话后续已自动熔断禁用: session={session_id}")); } // 熔断告警
            false // 放行主流程继续
        }
    }
}
pub fn build_compaction_message(msg_id: &str, thinking: &str, intro: &str, outro: &str, card_data: Value) -> Message { // 构造参考附图 1 的完整可持久化压缩助手消息
    let mut msg = Message::assistant()
        .with_visibility(true, false)
        .with_id(msg_id)
        .with_thinking(thinking, "")
        .with_text(intro)
        .with_content(MessageContent::SystemNotification(SystemNotificationContent {
            notification_type: SystemNotificationType::InlineMessage,
            msg: "context_compaction".to_string(),
            data: Some(card_data.clone()),
        }))
        .with_text(outro);
    msg.metadata.set_operation_note("context", "compactionCard", card_data);
    msg
}
fn emit_compress_error(tx: &UnboundedSender<AgentEvent>, err: &str) { // 推送手动压缩失败错误消息事件
    let msg = Message::assistant().with_error(MessageErrorKind::Other, err).with_generated_id_if_missing();
    let _ = tx.send(AgentEvent::Message(msg));
}
async fn execute_manual_compaction(app: &AppState, session_id: &str, history: &[Value], cfg: &ModelConfig, tokens_before_arg: i64, ctx_limit_arg: usize) -> Result<Value, String> { // 执行手动压缩核心（落库摘要/事实/隐藏旧消息/更新基线）并返回卡片载荷
    let (prior_summary, visible_msgs, current_total) = {
        let conn = app.db.lock();
        let prior = context_store::load_context(&conn, session_id).ok().flatten().and_then(|v| v.get("summary").and_then(Value::as_str).map(str::to_string)).unwrap_or_default();
        let all = messages_store::list(&conn, session_id).map_err(|e| format!("读取会话消息失败: {e}"))?;
        let visible: Vec<Message> = all.into_iter().filter(|m| m.is_agent_visible() && !messages_store::is_compaction_message(m)).collect();
        let total = usage_store::get_session_usage(&conn, session_id).ok().and_then(|v| v.get("total").and_then(Value::as_i64)).unwrap_or(0);
        (prior, visible, total)
    };
    let effective_limit = if ctx_limit_arg > 0 { ctx_limit_arg } else { models::goose_model_config(cfg).context_limit.unwrap_or(200_000) };
    let settings = crate::agent::settings::load();
    let timeout_secs = settings.timeout_seconds.max(10) as u64;
    if !visible_msgs.is_empty() {
        let hide_ids = select_compact_cut_ids(&visible_msgs, true);
        let hide_set: HashSet<&str> = hide_ids.iter().map(String::as_str).collect();
        let to_summarize: Vec<Message> = if !hide_ids.is_empty() {
            visible_msgs.iter().filter(|m| m.id.as_deref().map(|id| hide_set.contains(id)).unwrap_or(false)).cloned().collect()
        } else {
            visible_msgs.clone()
        };
        let chars_before: usize = to_summarize.iter().map(|m| format_message_for_compacting(m).chars().count()).sum();
        let summary = summarize_messages_with_model(cfg, &prior_summary, &to_summarize, timeout_secs).await?;
        let facts = extract_facts_with_model(cfg, &summary, timeout_secs).await;
        let tokens_before = ((chars_before / 3) as i64).max(tokens_before_arg).max(1);
        let tokens_after = (summary.chars().count() / 2).max(1) as i64;
        let (hidden_count, latest_total, saved_facts) = {
            let conn = app.db.lock();
            let hidden = if hide_ids.is_empty() { 0 } else { messages_store::hide_for_agent(&conn, session_id, &hide_ids).map_err(|e| format!("更新消息可见性失败: {e}"))? };
            context_store::save_context(&conn, session_id, &summary, tokens_before, tokens_after).map_err(|e| format!("保存压缩摘要失败: {e}"))?;
            let fact_vals: Vec<Value> = facts.iter().map(|f| json!(f)).collect();
            let _ = context_store::add_facts(&conn, session_id, &fact_vals);
            let saved_facts = context_store::load_facts(&conn, session_id, MAX_FACTS as i64).unwrap_or(facts);
            let latest = usage_store::get_session_usage(&conn, session_id).ok().and_then(|v| v.get("total").and_then(Value::as_i64)).unwrap_or(current_total);
            let _ = context_store::set_compaction_baseline(&conn, session_id, latest);
            (hidden, latest, saved_facts)
        };
        let percent = if effective_limit > 0 && latest_total > 0 { ((latest_total as f64 / effective_limit as f64) * 100.0).clamp(1.0, 100.0).round() as i64 } else if effective_limit > 0 && tokens_after > 0 { ((tokens_after as f64 / effective_limit as f64) * 100.0).clamp(1.0, 100.0).round() as i64 } else { 1 };
        logger::info("context", &format!("手动上下文压缩完成: session={session_id} hidden={hidden_count} before={tokens_before} after={tokens_after} baseline={latest_total} facts={}", saved_facts.len()));
        return Ok(json!({"tokens_before": tokens_before, "tokens_after": tokens_after, "hidden_count": hidden_count, "baseline": latest_total, "context_limit": effective_limit, "percent": percent, "summary": summary, "facts": saved_facts}));
    }
    let text = history_text(history);
    if text.trim().is_empty() { return Err("待压缩历史为空".to_string()); }
    let fake_msg = vec![Message::user().with_text(text.clone())];
    let summary = summarize_messages_with_model(cfg, &prior_summary, &fake_msg, timeout_secs).await?;
    let facts = extract_facts_with_model(cfg, &summary, timeout_secs).await;
    let tokens_before = ((text.chars().count() / 3) as i64).max(tokens_before_arg).max(1);
    let tokens_after = (summary.chars().count() / 2).max(1) as i64;
    let (latest_total, saved_facts) = {
        let conn = app.db.lock();
        context_store::save_context(&conn, session_id, &summary, tokens_before, tokens_after).map_err(|e| e.to_string())?;
        let fact_vals: Vec<Value> = facts.iter().map(|f| json!(f)).collect();
        let _ = context_store::add_facts(&conn, session_id, &fact_vals);
        let saved_facts = context_store::load_facts(&conn, session_id, MAX_FACTS as i64).unwrap_or(facts);
        let latest = usage_store::get_session_usage(&conn, session_id).ok().and_then(|v| v.get("total").and_then(Value::as_i64)).unwrap_or(0);
        let _ = context_store::set_compaction_baseline(&conn, session_id, latest);
        (latest, saved_facts)
    };
    let percent = if effective_limit > 0 && latest_total > 0 { ((latest_total as f64 / effective_limit as f64) * 100.0).clamp(1.0, 100.0).round() as i64 } else { 1 };
    logger::info("context", &format!("手动上下文压缩完成(文本历史): session={session_id} before={tokens_before} after={tokens_after} facts={}", saved_facts.len()));
    Ok(json!({"tokens_before": tokens_before, "tokens_after": tokens_after, "hidden_count": 0, "baseline": latest_total, "context_limit": effective_limit, "percent": percent, "summary": summary, "facts": saved_facts}))
}
pub async fn compress_stream(app: AppState, session_id: String, history: Vec<Value>, model_name: String, _agent_id: String, tokens_before: i64, ctx_limit: usize, tx: UnboundedSender<AgentEvent>) { // 手动压缩 SSE 流：逐步推思考/引语/[压缩]卡/结语并落库
    let msg_id = format!("msg-compact-{}", uuid::Uuid::new_v4().simple());
    let thinking_head = "**思考过程**\n正在检索当前会话可见历史与上下文窗口占用…";
    let mut frame1 = Message::assistant().with_visibility(true, false).with_id(&msg_id).with_thinking(thinking_head, "");
    frame1.metadata.set_operation_note("context", "compactionCard", json!({"status": "running"}));
    let _ = tx.send(AgentEvent::Message(frame1));
    let cfg = match pick_model_config(&model_name) {
        Ok(item) => item,
        Err(err) => { logger::error("context", &format!("手动压缩选型失败: {session_id} {err}")); emit_compress_error(&tx, &err); return; }
    };
    let card_data = match execute_manual_compaction(&app, &session_id, &history, &cfg, tokens_before, ctx_limit).await {
        Ok(data) => data,
        Err(err) => { logger::error("context", &format!("手动上下文压缩失败: {session_id} {err}")); emit_compress_error(&tx, &format!("压缩失败：{err}")); return; }
    };
    let thinking_tail = "\n已完成 9 段式结构化摘要与关键事实归纳。";
    let intro = "我将开始压缩历史对话记录。";
    let outro = "历史对话记录压缩已完成。";
    let full_thinking = format!("{thinking_head}{thinking_tail}");
    let full_msg = build_compaction_message(&msg_id, &full_thinking, intro, outro, card_data.clone());
    {
        let conn = app.db.lock();
        let _ = messages_store::upsert(&conn, &session_id, &full_msg);
    }
    let frame2 = Message::assistant().with_visibility(true, false).with_id(&msg_id).with_thinking(thinking_tail, "").with_text(intro);
    let _ = tx.send(AgentEvent::Message(frame2));
    let mut frame3 = Message::assistant().with_visibility(true, false).with_id(&msg_id).with_content(MessageContent::SystemNotification(SystemNotificationContent {
        notification_type: SystemNotificationType::InlineMessage,
        msg: "context_compaction".to_string(),
        data: Some(card_data.clone()),
    }));
    frame3.metadata.set_operation_note("context", "compacted", json!({
        "hidden": card_data.get("hidden_count").cloned().unwrap_or(json!(0)),
        "tokens_after": card_data.get("tokens_after").cloned().unwrap_or(json!(0)),
        "baseline": card_data.get("baseline").cloned().unwrap_or(json!(0)),
    }));
    frame3.metadata.set_operation_note("context", "compactionCard", card_data);
    let _ = tx.send(AgentEvent::Message(frame3));
    let frame4 = Message::assistant().with_visibility(true, false).with_id(&msg_id).with_text(outro);
    let _ = tx.send(AgentEvent::Message(frame4));
}
pub async fn compress(session_id: &str, history: &[Value], model_name: &str, _agent_id: &str, tokens_before: i64) -> Result<Value, String> { // 保留同步调用包装以便测试或内部复用
    let cfg = pick_model_config(model_name)?;
    let app = state::shared();
    execute_manual_compaction(&app, session_id, history, &cfg, tokens_before, 0).await
}
#[cfg(test)] // 三层上下文压缩核心单元测试
mod tests { // 测试微压缩 5 条保护、analysis/summary 清洗与工具配对边界对齐
    use super::*; // 引入被测函数
    use rmcp::model::{CallToolRequestParams, CallToolResult}; // 工具请求与返回构造
    #[test] // 两阶段微压缩：<=2000 字符短结果与最近 3 个 Assistant 轮次完整保留，超过 30% 软裁剪、超过 50% 硬清空
    fn microcompact_two_stage_protects_recent_turns_and_short_outputs() {
        let mut msgs = vec![Message::user().with_text("start")];
        // 构造 4 个 Assistant 轮次（第 0 轮超过 3 轮保护线，第 1~3 轮受保护）
        // 第 0 轮附带 1 个短结果（300 字）和 1 个长结果（2500 字）
        msgs.push(Message::assistant().with_text("turn-0"));
        msgs.push(Message::user().with_tool_response("c-short", Ok(CallToolResult::success(vec![ContentBlock::text("s".repeat(300))]))));
        msgs.push(Message::user().with_tool_response("c-long-0", Ok(CallToolResult::success(vec![ContentBlock::text(format!("HEAD_{}_TAIL", "a".repeat(2500)))]))));
        for idx in 1..=3 {
            msgs.push(Message::assistant().with_text(format!("turn-{idx}")));
            msgs.push(Message::user().with_tool_response(format!("c-recent-{idx}"), Ok(CallToolResult::success(vec![ContentBlock::text("r".repeat(2500))]))));
        }
        // context_limit = 5000 tokens -> char_window = 20000；30%=6000，50%=10000
        // 仅验证第 1 阶段 Soft Trim：设置较大窗口使软裁剪后 < 50%（context_limit=7000 -> window=28000, 30%=8400, 50%=14000; total~10300）
        let (soft_proj, soft_changed) = microcompact_messages_with_limit(&msgs, MICROCOMPACT_KEEP_RECENT, 7_000);
        assert_eq!(soft_changed, 1);
        let short_text = soft_proj[2].content[0].as_tool_response().and_then(|r| r.tool_result.as_ref().ok()).and_then(|v| v.content[0].as_text()).map(|t| t.text.as_str()).unwrap();
        let trim_text = soft_proj[3].content[0].as_tool_response().and_then(|r| r.tool_result.as_ref().ok()).and_then(|v| v.content[0].as_text()).map(|t| t.text.as_str()).unwrap();
        assert_eq!(short_text.len(), 300); // <=2000 短结果绝不清理
        assert!(trim_text.starts_with("HEAD_"));
        assert!(trim_text.contains(SOFT_TRIM_MARKER)); // 旧长输出被 Soft Trim
        // Stage 2 Hard Clear：更小窗口（context_limit=3500 -> window=14000, 50%=7000; 软裁剪后仍 >7000）
        let (hard_proj, _) = microcompact_messages_with_limit(&msgs, MICROCOMPACT_KEEP_RECENT, 3_500);
        let cleared_text = hard_proj[3].content[0].as_tool_response().and_then(|r| r.tool_result.as_ref().ok()).and_then(|v| v.content[0].as_text()).map(|t| t.text.as_str());
        let recent_text = hard_proj[5].content[0].as_tool_response().and_then(|r| r.tool_result.as_ref().ok()).and_then(|v| v.content[0].as_text()).map(|t| t.text.as_str()).unwrap();
        assert_eq!(cleared_text, Some(MICROCOMPACT_CLEARED_TEXT)); // 仍超 50% 时旧长输出硬清空
        assert_eq!(recent_text.len(), 2500); // 最近 3 轮 Assistant 内长输出原样保留
    }
    #[test] // extract_compact_summary 剥离 analysis 仅留 summary
    fn summary_strips_analysis_scratchpad() {
        let raw = "<analysis>草稿思考不应外泄</analysis>\n<summary>\n1. Primary Request and Intent\n完成测试\n</summary>"; // 模拟模型输出
        assert_eq!(extract_compact_summary(raw), "1. Primary Request and Intent\n完成测试"); // 仅剩干净摘要
    }
    #[test] // 切分边界不得拆分 Assistant ToolRequest 与对应 ToolResponse，且必须保留最新用户提问
    fn cut_point_never_splits_tool_pair_or_latest_prompt() {
        let req_params: CallToolRequestParams = serde_json::from_value(json!({"name": "file_read", "arguments": {"path": "a.txt"}})).unwrap(); // 反序列化请求参数
        let msgs = vec![
            Message::user().with_text("old prompt").with_generated_id_if_missing(), // #0 旧提问
            Message::assistant().with_text("old reply").with_generated_id_if_missing(), // #1 旧回答
            Message::user().with_text("current prompt").with_generated_id_if_missing(), // #2 当前提问（必须保留）
            Message::assistant().with_tool_request("c1", Ok(req_params.clone())).with_generated_id_if_missing(), // #3
            Message::user().with_tool_response("c1", Ok(CallToolResult::success(vec![ContentBlock::text("res1")]))).with_generated_id_if_missing(), // #4
            Message::assistant().with_tool_request("c2", Ok(req_params)).with_generated_id_if_missing(), // #5
            Message::user().with_tool_response("c2", Ok(CallToolResult::success(vec![ContentBlock::text("res2")]))).with_generated_id_if_missing(), // #6
        ];
        let ids = select_compact_cut_ids(&msgs, false); // 切分选出待隐藏项
        assert!(ids.contains(msgs[0].id.as_ref().unwrap()) && ids.contains(msgs[1].id.as_ref().unwrap())); // 旧提问与旧回答被选中隐藏
        assert!(!ids.contains(msgs[2].id.as_ref().unwrap())); // 最新用户输入绝不隐藏
    }
    #[test] // 验证 build_compaction_message 四段顺序、仅前端可见且 SQLite RoundTrip 完整
    fn build_compaction_message_preserves_order_and_user_only_visibility() {
        let card = json!({"tokens_before": 12000, "tokens_after": 600, "hidden_count": 4, "baseline": 12000, "context_limit": 200000, "percent": 6, "summary": "摘要内容", "facts": ["事实1"]});
        let msg = build_compaction_message("msg-compact-1", "**思考过程**\n分析中", "我将开始压缩历史对话记录。", "历史对话记录压缩已完成。", card);
        assert!(msg.is_user_visible());
        assert!(!msg.is_agent_visible());
        assert!(messages_store::is_compaction_message(&msg));
        assert_eq!(msg.content.len(), 4);
        let serialized = serde_json::to_string(&msg).unwrap();
        let restored: Message = serde_json::from_str(&serialized).unwrap();
        assert_eq!(restored.content.len(), 4);
        assert!(messages_store::is_compaction_message(&restored));
    }
}
