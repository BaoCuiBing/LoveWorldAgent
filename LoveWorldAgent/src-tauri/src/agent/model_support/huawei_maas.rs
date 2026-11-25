use std::sync::Arc; // 华为云 MaaS 标准 API V2 适配器：https://api.modelarts-maas.com/v2/chat/completions
use async_trait::async_trait; // 异步 trait 宏
use goose_provider_types::base::{MessageStream, Provider}; // goose provider 抽象
use goose_provider_types::conversation::message::Message; // 会话消息类型
use goose_provider_types::errors::ProviderError; // provider 错误类型
use goose_provider_types::formats::openai::create_request; // OpenAI 兼容消息/工具请求构造
use goose_provider_types::images::ImageFormat; // 图片格式定义
use goose_provider_types::model::ModelConfig as GooseModelConfig; // goose 模型配置
use goose_provider_types::request_log::{start_log, LoggerHandleExt}; // 请求日志记录
use goose_provider_types::thinking::ThinkingEffort; // 思考强度枚举
use goose_providers::api_client::ApiClient; // HTTP 客户端
use goose_providers::openai_compatible::{handle_response, handle_status, stream_openai_compat}; // SSE 流与状态码处理
use goose_providers::retry::ProviderRetry; // 重试策略
use rmcp::model::Tool; // MCP 工具定义
use serde_json::{json, Map, Value}; // JSON 构造与操作
use super::openapi::build_client; // 复用 TLS/Bearer 客户端构造
use super::ModelConfig; // 应用内模型配置
pub const PROVIDER_KEY: &str = "huawei"; // 华为 MaaS 服务商标识
pub const PROVIDER_DISPLAY: &str = "华为"; // 服务商展示名
pub const DEFAULT_BASE_URL: &str = "https://api.modelarts-maas.com/v2"; // 华为云 MaaS 标准 API V2 根地址
pub struct MaasModelMeta { // 华为云 MaaS 预置文本模型元信息（依据 MaaS 标准 API V2 官方文档）
    pub id: &'static str, // 模型 ID
    pub max_context: usize, // 实测上下文上限
    pub context_options: &'static [&'static str], // 前端可选上下文档位
    pub default_context: &'static str, // 默认上下文档位
    pub vision: bool, // 是否支持视觉
    pub thinking_title: &'static str, // 前端参数项标题（深度思考 / 思考强度）
    pub thinking_options: &'static [(&'static str, &'static str)], // 前端可选思考参数（value, label）
    pub default_thinking: &'static str, // 默认思考档位
    pub thinking_always_on: bool, // 是否强制开启深度思考
    pub uses_reasoning_effort: bool, // 是否支持 reasoning_effort 字段
    pub supports_penalties: bool, // 是否支持修改 frequency_penalty/presence_penalty
}
const OP_THINKING: &[(&str, &str)] = &[("enabled", "开启"), ("disabled", "关闭")]; // openPangu 仅支持开关
const GLM53_THINKING: &[(&str, &str)] = &[("low", "低"), ("high", "高"), ("max", "极高")]; // GLM-5.3 强制思考，支持 low/high/max
const EFFORT_THINKING: &[(&str, &str)] = &[("off", "关闭"), ("low", "低"), ("high", "高"), ("max", "极高")]; // GLM-5.2 / DeepSeek V4.1 Flash 支持开关与三档强度
const CTX_512K: &[&str] = &["200k", "600k"]; // openPangu 上下文挡位（内部裁剪至 512k）
const CTX_198K: &[&str] = &["200k"]; // glm-5.2-arkts-spark 上下文挡位（内部裁剪至 198k）
const CTX_1M: &[&str] = &["200k", "600k", "1M"]; // 1M 级模型上下文档位
pub const FEATURED_MODELS: &[MaasModelMeta] = &[ // 华为云 MaaS 主推文本模型清单（支持 Function Call 与思维链，已剔除 2.0 停售下线模型）
    MaasModelMeta { id: "openpangu-2.1-pro", max_context: 512_000, context_options: CTX_512K, default_context: "600k", vision: false, thinking_title: "深度思考", thinking_options: OP_THINKING, default_thinking: "enabled", thinking_always_on: false, uses_reasoning_effort: false, supports_penalties: false }, // 盘古 2.1 Pro（penalty 固定 0）
    MaasModelMeta { id: "glm-5.3", max_context: 1_000_000, context_options: CTX_1M, default_context: "1M", vision: false, thinking_title: "思考强度", thinking_options: GLM53_THINKING, default_thinking: "high", thinking_always_on: true, uses_reasoning_effort: true, supports_penalties: true }, // GLM-5.3（固定开启思考）
    MaasModelMeta { id: "glm-5.2", max_context: 1_000_000, context_options: CTX_1M, default_context: "1M", vision: false, thinking_title: "思考强度", thinking_options: EFFORT_THINKING, default_thinking: "high", thinking_always_on: false, uses_reasoning_effort: true, supports_penalties: true }, // GLM-5.2
    MaasModelMeta { id: "glm-5.2-arkts-spark", max_context: 198_000, context_options: CTX_198K, default_context: "200k", vision: false, thinking_title: "思考强度", thinking_options: EFFORT_THINKING, default_thinking: "high", thinking_always_on: false, uses_reasoning_effort: true, supports_penalties: true }, // GLM-5.2 ArkTS Spark
    MaasModelMeta { id: "deepseek-v4.1-flash-1017", max_context: 1_000_000, context_options: CTX_1M, default_context: "1M", vision: true, thinking_title: "思考强度", thinking_options: EFFORT_THINKING, default_thinking: "high", thinking_always_on: false, uses_reasoning_effort: true, supports_penalties: false }, // DeepSeek V4.1 Flash 1017
    MaasModelMeta { id: "deepseek-v4.1-flash", max_context: 1_000_000, context_options: CTX_1M, default_context: "1M", vision: true, thinking_title: "思考强度", thinking_options: EFFORT_THINKING, default_thinking: "high", thinking_always_on: false, uses_reasoning_effort: true, supports_penalties: false }, // DeepSeek V4.1 Flash
];
pub fn find_model_meta(model: &str) -> Option<&'static MaasModelMeta> { // 按模型名精确或最长前缀匹配元信息
    let target = model.trim().to_lowercase(); // 归一化模型名
    FEATURED_MODELS.iter().find(|meta| meta.id == target).or_else(|| FEATURED_MODELS.iter().filter(|meta| target.starts_with(meta.id)).max_by_key(|meta| meta.id.len())) // 先精确后最长前缀匹配
}
pub fn model_names() -> Vec<String> { // 全部预置文本模型名列表
    FEATURED_MODELS.iter().map(|meta| meta.id.to_string()).collect() // 收集模型 ID
}
pub fn clamp_context_limit(model: &str, configured: usize) -> usize { // 按 MaaS 模型实际窗口裁剪上下文上限
    find_model_meta(model).map(|meta| configured.min(meta.max_context)).unwrap_or(configured) // 未知模型保持配置值
}
pub fn default_vision(model: &str) -> bool { // 返回预置模型默认视觉能力
    find_model_meta(model).map(|meta| meta.vision).unwrap_or(false) // 缺省不支持
}
pub fn context_options(model: &str) -> &'static [&'static str] { // 返回模型支持的上下文档位列表
    find_model_meta(model).map(|meta| meta.context_options).unwrap_or(CTX_1M) // 未知回退全量档位
}
pub fn default_context(model: &str) -> &'static str { // 返回模型默认上下文档位
    find_model_meta(model).map(|meta| meta.default_context).unwrap_or("600k") // 缺省 600k
}
pub fn thinking_title(model: &str) -> &'static str { // 返回模型思考参数展示标题
    find_model_meta(model).map(|meta| meta.thinking_title).unwrap_or("深度思考") // 未知按开关标题
}
pub fn thinking_options(model: &str) -> &'static [(&'static str, &'static str)] { // 返回模型支持的思考参数列表
    find_model_meta(model).map(|meta| meta.thinking_options).unwrap_or(OP_THINKING) // 未知模型安全回退开关模式
}
pub fn default_thinking(model: &str) -> &'static str { // 返回模型默认思考参数值
    find_model_meta(model).map(|meta| meta.default_thinking).unwrap_or("enabled") // 缺省开启
}
pub fn normalize_context(model: &str, raw: &str) -> String { // 归一化华为模型上下文档位
    let target = raw.trim().to_lowercase(); // 小写比对
    if let Some(hit) = context_options(model).iter().find(|opt| opt.to_lowercase() == target) { return (*hit).to_string(); } // 命中合法档位返回规范写法
    default_context(model).to_string() // 非法值回退模型默认档位
}
pub fn normalize_thinking(model: &str, raw: &str) -> String { // 归一化华为模型思考参数，兼容历史 medium 值
    let val = raw.trim().to_lowercase(); // 小写比对
    let opts = thinking_options(model); // 模型合法选项
    if opts.iter().any(|(key, _)| *key == val) { return val; } // 直接命中合法选项
    let meta = find_model_meta(model); // 查元信息
    if meta.map(|item| !item.uses_reasoning_effort).unwrap_or(true) { // 仅开关模式（openPangu 与未知模型）
        return if matches!(val.as_str(), "disabled" | "off" | "none" | "false") { "disabled".to_string() } else { "enabled".to_string() }; // 映射到开关值
    }
    if meta.map(|item| item.thinking_always_on).unwrap_or(false) { // GLM-5.3 强制开启
        return match val.as_str() { "low" | "off" | "disabled" | "none" => "low", "max" | "xhigh" => "max", _ => "high" }.to_string(); // 历史 medium 归入 high
    }
    match val.as_str() { "off" | "disabled" | "none" => "off", "low" => "low", "max" | "xhigh" => "max", _ => "high" }.to_string() // 其余模型：历史 medium 归入 high
}
pub struct HuaweiMaasProvider { // 华为云 MaaS Standard API V2 Provider
    api_client: ApiClient, // HTTP 客户端
}
impl HuaweiMaasProvider { // 构造方法与请求体规范化
    pub fn new(api_client: ApiClient) -> Self { // 创建 MaaS Provider
        Self { api_client } // 保存客户端
    }
    fn sanitize_payload(payload: &mut Value, model_config: &GooseModelConfig) { // 按 MaaS V2 规范修正请求体字段
        let Some(obj) = payload.as_object_mut() else { return; }; // 取根对象
        if let Some(messages) = obj.get_mut("messages").and_then(Value::as_array_mut) { // 遍历消息数组
            for message in messages { // 规范化消息角色与空内容
                if message.get("role").and_then(Value::as_str) == Some("developer") { message["role"] = Value::String("system".to_string()); } // MaaS 仅识别 system
                if message.get("role").and_then(Value::as_str) == Some("assistant") && message.get("tool_calls").is_some() && matches!(message.get("content"), None | Some(Value::Null)) { message["content"] = Value::String(String::new()); } // 工具调用历史消息保证 content 为空串而非 null
            }
        }
        if obj.contains_key("max_tokens") && obj.contains_key("max_completion_tokens") { obj.remove("max_completion_tokens"); } // 两字段并存会报 400，保留 max_tokens
        obj.remove("chat_template_kwargs"); // MaaS V2 使用顶层 thinking，不传 V1 参数
        if find_model_meta(&model_config.model_name).map(|item| !item.supports_penalties).unwrap_or(false) { // 移除模型不支持的惩罚参数
            obj.remove("frequency_penalty"); // 避免 DeepSeek/盘古 2.1 传不支持参数报错
            obj.remove("presence_penalty"); // 同上
        }
        Self::apply_thinking(obj, model_config); // 写入 thinking 与 reasoning_effort
    }
    fn apply_thinking(obj: &mut Map<String, Value>, model_config: &GooseModelConfig) { // 写入 MaaS V2 深度思考参数
        let meta = find_model_meta(&model_config.model_name); // 查模型元信息
        let effort = model_config.thinking_effort(); // 解析 goose 思考强度
        let always_on = meta.map(|item| item.thinking_always_on).unwrap_or(false); // 是否强制开启
        let enabled = always_on || !matches!(effort, Some(ThinkingEffort::Off)); // 计算思考开关
        obj.insert("thinking".to_string(), json!({"type": if enabled { "enabled" } else { "disabled" }})); // MaaS V2 标准开关
        let supports_effort = meta.map(|item| item.uses_reasoning_effort).unwrap_or(false); // 是否支持强度枚举
        if !supports_effort || !enabled { obj.remove("reasoning_effort"); return; } // 不支持或关闭时移除
        let level = match effort.unwrap_or(ThinkingEffort::High) { // 按 MaaS V2 规范映射到 low/high/max（DeepSeek/GLM-5.3 均不支持 medium）
            ThinkingEffort::Off | ThinkingEffort::Low => "low", // 轻量推理
            ThinkingEffort::Medium | ThinkingEffort::High => "high", // 增强推理（含兼容 medium→high）
            ThinkingEffort::Max => "max", // 深度推理
        };
        obj.insert("reasoning_effort".to_string(), Value::String(level.to_string())); // 写入思考强度
    }
}
fn decorate_maas_error(err: ProviderError) -> ProviderError { // 为 MaaS 常见 403 报错补充中文指引
    match err {
        ProviderError::Authentication(msg) if msg.contains("Invalid request because you do not have access to it") => ProviderError::Authentication(format!("{msg}（请确认已在华为云「西南-贵阳一」MaaS 控制台开通对应预置服务且 API Key 权限允许该模型）")), // 补充 ModelArts.81004 说明
        ProviderError::Authentication(msg) if msg.to_ascii_lowercase().contains("resource is frozen") => ProviderError::Authentication(format!("{msg}（华为云 MaaS 资源已冻结：请检查账号欠费状态、免费额度是否耗尽或 2.0 旧模型是否已下线，建议充值/解冻后切换 openpangu-2.1-pro / deepseek-v4.1-flash-1017）")), // 补充资源冻结说明
        other => other, // 其他错误原样透传
    }
}
#[async_trait]
impl Provider for HuaweiMaasProvider { // 实现 goose Provider trait
    fn get_name(&self) -> &str { // 返回服务商名
        PROVIDER_KEY // 固定返回 huawei
    }
    async fn refresh_credentials(&self) -> Result<(), ProviderError> { // 刷新凭证
        self.api_client.refresh_credentials().await.map_err(|err| ProviderError::Authentication(err.to_string())) // 透传客户端刷新逻辑
    }
    async fn fetch_supported_models(&self) -> Result<Vec<String>, ProviderError> { // 拉取 MaaS V2 模型列表，失败回退预置清单
        let Ok(response) = self.api_client.response_get("models").await else { return Ok(model_names()); }; // 网络异常回退预置文本模型
        let Ok(json) = handle_response(response).await else { return Ok(model_names()); }; // 解析失败回退预置
        let Some(arr) = json.get("data").and_then(Value::as_array) else { return Ok(model_names()); }; // 取 data 数组
        let mut models: Vec<String> = arr.iter().filter_map(|item| item.get("id").and_then(Value::as_str).map(str::to_string)).collect(); // 提取模型 ID
        for name in model_names() { if !models.contains(&name) { models.push(name); } } // 合并主推模型清单
        models.sort(); // 排序返回
        Ok(models) // 返回模型集合
    }
    async fn stream(&self, model_config: &GooseModelConfig, system: &str, messages: &[Message], tools: &[Tool]) -> Result<MessageStream, ProviderError> { // 发起 SSE 流式对话请求
        let mut payload = create_request(model_config, system, messages, tools, &ImageFormat::OpenAi, true) // 构造基础请求体
            .map_err(|err| ProviderError::RequestFailed(format!("构造 MaaS 请求失败: {err}")))?; // 转换构造错误
        Self::sanitize_payload(&mut payload, model_config); // 套用 MaaS V2 规范字段
        let mut log = start_log(model_config, &payload)?; // 打开请求日志
        let response = self.with_retry(|| async { // 按 goose 策略自动重试瞬态错误
            handle_status(self.api_client.request("chat/completions").model_headers(model_config)?.streaming(true).response_post(&payload).await?).await // 发送 POST 请求并校验 HTTP 状态
        }).await.map_err(decorate_maas_error).inspect_err(|err| { // 记录失败日志到 app.log
            crate::utils::logger::error("huawei_maas", &format!("MaaS 请求失败 model={}: {err}", model_config.model_name)); // 写入主日志文件便于排障
            let _ = log.error(err); // 同步写入请求日志
        })?;
        stream_openai_compat(response, log) // 复用 OpenAI 兼容 SSE 流解码（含 reasoning_content/tool_calls/usage）
    }
}
pub fn build_provider(config: &ModelConfig) -> Result<Arc<dyn Provider>, String> { // 构造华为云 MaaS Provider 实例
    let mut normalized = config.clone(); // 克隆配置以便填补默认地址
    if normalized.base_url.trim().is_empty() { normalized.base_url = DEFAULT_BASE_URL.to_string(); } // 缺省使用 MaaS V2 标准地址
    if normalized.api_key.trim().is_empty() { return Err("华为 MaaS 模型 api_key 不能为空".to_string()); } // 未填 Key 明确报错提示
    let client = build_client(&normalized, PROVIDER_DISPLAY)?; // 创建 TLS 客户端
    Ok(Arc::new(HuaweiMaasProvider::new(client))) // 返回华为 MaaS Provider
}
#[cfg(test)] // 华为云 MaaS V2 请求规范单元测试
mod tests { // 测试 thinking、reasoning_effort、max_tokens 去重、惩罚清理、参数选项与上下文裁剪
    use super::*; // 引入被测函数
    #[test] // glm-5.3 强制开启 thinking，Max 映射为 max，且历史 medium 映射为 high
    fn glm53_keeps_thinking_enabled_and_maps_efforts() {
        let mut payload = json!({"model": "glm-5.3", "messages": [{"role": "developer", "content": "hi"}, {"role": "assistant", "content": null, "tool_calls": [{"id": "1"}]}], "max_tokens": 4096, "max_completion_tokens": 4096}); // 构造请求体
        let cfg = GooseModelConfig::new("glm-5.3").with_default_thinking_effort(Some(ThinkingEffort::Off)); // 尝试关闭
        HuaweiMaasProvider::sanitize_payload(&mut payload, &cfg); // 规范化
        assert_eq!(payload["thinking"], json!({"type": "enabled"})); // 不可关闭
        assert_eq!(payload["reasoning_effort"], json!("low")); // Off 映射到 low
        assert_eq!(payload["messages"][0]["role"], json!("system")); // developer 转 system
        assert_eq!(payload["messages"][1]["content"], json!("")); // tool_calls 消息 null 转空串
        assert!(payload.get("max_completion_tokens").is_none()); // 移除重复上限字段
        let cfg_med = GooseModelConfig::new("glm-5.3").with_default_thinking_effort(Some(ThinkingEffort::Medium)); // 中档测试
        HuaweiMaasProvider::sanitize_payload(&mut payload, &cfg_med); // 规范化
        assert_eq!(payload["reasoning_effort"], json!("high")); // Medium 映射到 high
        let cfg_max = GooseModelConfig::new("glm-5.3").with_default_thinking_effort(Some(ThinkingEffort::Max)); // 极高档测试
        HuaweiMaasProvider::sanitize_payload(&mut payload, &cfg_max); // 规范化
        assert_eq!(payload["reasoning_effort"], json!("max")); // Max 映射到 max
    }
    #[test] // deepseek-v4.1-flash* 仅传 low/high/max 或关闭 thinking，并清理不支持的惩罚参数
    fn deepseek_maas_never_emits_medium_and_strips_penalties() {
        let mut payload = json!({"model": "deepseek-v4.1-flash-1017", "messages": [{"role": "user", "content": "hi"}], "frequency_penalty": 0.2, "presence_penalty": 0.1}); // 构造请求体
        let cfg_med = GooseModelConfig::new("deepseek-v4.1-flash-1017").with_default_thinking_effort(Some(ThinkingEffort::Medium)); // 历史 medium
        HuaweiMaasProvider::sanitize_payload(&mut payload, &cfg_med); // 规范化
        assert_eq!(payload["thinking"], json!({"type": "enabled"})); // 开启 thinking
        assert_eq!(payload["reasoning_effort"], json!("high")); // 不再发出非法 medium
        assert!(payload.get("frequency_penalty").is_none() && payload.get("presence_penalty").is_none()); // 移除惩罚字段
        let cfg_off = GooseModelConfig::new("deepseek-v4.1-flash-1017").with_default_thinking_effort(Some(ThinkingEffort::Off)); // 关闭思考
        HuaweiMaasProvider::sanitize_payload(&mut payload, &cfg_off); // 规范化
        assert_eq!(payload["thinking"], json!({"type": "disabled"})); // 关闭 thinking
        assert!(payload.get("reasoning_effort").is_none()); // 关闭时不发 reasoning_effort
    }
    #[test] // openpangu 不传 reasoning_effort 且上下文上限裁剪至 512k
    fn openpangu_omits_reasoning_effort_and_clamps_context() {
        let mut payload = json!({"model": "openpangu-2.1-pro", "messages": [{"role": "system", "content": "s"}], "reasoning_effort": "medium"}); // 构造请求体
        let cfg = GooseModelConfig::new("openpangu-2.1-pro").with_default_thinking_effort(Some(ThinkingEffort::High)); // 开启档
        HuaweiMaasProvider::sanitize_payload(&mut payload, &cfg); // 规范化
        assert_eq!(payload["thinking"], json!({"type": "enabled"})); // 开启 thinking
        assert!(payload.get("reasoning_effort").is_none()); // 不支持则移除
        assert_eq!(clamp_context_limit("openpangu-2.1-pro", 600_000), 512_000); // 600k 档位裁剪至 512k
        assert_eq!(clamp_context_limit("glm-5.2-arkts-spark", 200_000), 198_000); // 200k 档位裁剪至 198k
        assert_eq!(normalize_context("openpangu-2.1-pro", "1M"), "600k"); // 非法 1M 归一到 600k
        assert_eq!(normalize_thinking("openpangu-2.1-pro", "high"), "enabled"); // 历史 high 归一到 enabled
        assert_eq!(normalize_thinking("openpangu-2.1-pro", "off"), "disabled"); // off 归一到 disabled
    }
    #[test] // 服务商聚合包含华为及全部 6 个主推模型（已移除 2.0 下线模型）
    fn providers_json_includes_huawei_preset() {
        let providers = super::super::providers_json(&[]); // 空配置也应包含华为预设
        let huawei = providers.iter().find(|entry| entry["value"] == "huawei").expect("missing huawei preset"); // 查找华为项
        assert_eq!(huawei["display"], json!("华为")); // 显示名
        assert_eq!(huawei["base_url"], json!(DEFAULT_BASE_URL)); // V2 地址
        assert_eq!(huawei["models"].as_array().map(Vec::len), Some(6)); // 6 个主推文本模型
        let frozen = decorate_maas_error(ProviderError::Authentication("Status: 403 Forbidden. Response: The resource is frozen.".to_string())); // 冻结错误
        assert!(frozen.to_string().contains("资源已冻结")); // 附带中文排查提示
    }
}
