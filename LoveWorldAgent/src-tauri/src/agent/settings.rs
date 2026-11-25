use std::path::PathBuf; // Agent 运行设置：合并 agent_config.json 的 agent 段，定位可写 relus/skills/agents 目录并播种随包资源
use serde_json::Value; // 配置动态值
use crate::agent::{agents_repo, models, rules, skills}; // 规则/技能/智能体仓库与模型解析
use crate::config; // 配置路径与加载
#[derive(Clone)] // 设置可克隆
pub struct AgentSettings { // Agent 运行设置集合
    pub enabled: bool, // 是否启用智能体
    pub model: String, // 当前模型名
    pub max_turns: i64, // 最大轮次
    pub timeout_seconds: i64, // 整体超时秒数
    pub command_timeout_seconds: i64, // 命令超时秒数
    pub base_url: String, // 模型 API 地址
    pub api_key: String, // 模型 API Key
    pub rule_dir: PathBuf, // 可写规则目录
    pub skill_dir: PathBuf, // 可写技能目录
}
pub fn load() -> AgentSettings { // 合并配置并播种随包资源，返回运行设置
    let config_data = config::load_agent_config(); // 加载 agent_config.json
    let section = config_data.get("agent").and_then(Value::as_object).cloned().unwrap_or_default(); // 取 agent 配置段
    let active = models::active_model(config_data.get("llm").unwrap_or(&Value::Null)); // 解析当前激活模型
    let rule_dir = config::WRITABLE_LOVEWORD_DIR.join("relus"); // 可写规则目录
    let skill_dir = config::WRITABLE_LOVEWORD_DIR.join("skills"); // 可写技能目录
    rules::seed_rules(&rule_dir, &config::LOVEWORD_DIR.join("relus")); // 若为空则播种随包规则
    skills::seed_skills(&skill_dir, &config::LOVEWORD_DIR.join("skills")); // 若为空则播种随包技能
    agents_repo::seed_agents(&config::WRITABLE_LOVEWORD_DIR.join("agents"), &config::LOVEWORD_DIR.join("agents")); // 若为空则播种随包子智能体
    let _ = std::fs::create_dir_all(&rule_dir); // 确保规则目录存在
    let _ = std::fs::create_dir_all(&skill_dir); // 确保技能目录存在
    let model = section.get("model").and_then(Value::as_str).map(|text| text.to_string()).filter(|text| !text.is_empty()) // 配置里的模型名
        .or_else(|| active.as_ref().map(|item| item.model.clone())).filter(|text| !text.is_empty()).unwrap_or_else(|| "deepseek-chat".to_string()); // 回退激活模型，再回退默认
    AgentSettings { // 组装最终设置
        enabled: section.get("enabled").and_then(Value::as_bool).unwrap_or(true), // 缺省启用
        model, // 已解析的模型名
        max_turns: section.get("max_turns").and_then(Value::as_i64).unwrap_or(200), // 缺省 200 轮
        timeout_seconds: section.get("timeout_seconds").and_then(Value::as_i64).unwrap_or(180), // 缺省 180 秒
        command_timeout_seconds: section.get("command_timeout_seconds").and_then(Value::as_i64).unwrap_or(60), // 缺省 60 秒
        base_url: active.as_ref().map(|item| item.base_url.clone()).unwrap_or_else(|| "https://api.deepseek.com/v1".to_string()), // API 地址回退默认
        api_key: active.map(|item| item.api_key).unwrap_or_default(), // API Key 缺省空
        rule_dir, // 可写规则目录
        skill_dir, // 可写技能目录
    }
}
