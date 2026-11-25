pub mod huawei_maas; // 华为云 MaaS 标准 API V2 适配（openpangu / glm-5.x / deepseek-v4.1 等文本模型）
pub mod openapi; // OpenAI 兼容协议模型构建（DeepSeek / Kimi / GLM / 自定义端点）
use serde_json::Value; // JSON 类型
use std::sync::Arc; // 引用计数智能指针
use goose_provider_types::base::Provider as GooseProvider; // goose provider trait
use goose_provider_types::model::ModelConfig as GooseModelConfig; // goose 模型配置类型
use goose_provider_types::thinking::ThinkingEffort; // goose 思考强度枚举
pub const CONTEXT_OPTIONS: [&str; 3] = ["200k", "600k", "1M"]; // 默认可选上下文窗口档位
pub const THINKING_OPTIONS: [(&str, &str); 3] = [("low", "低"), ("medium", "中"), ("high", "高")]; // 默认可选思考强度档位
pub const DEFAULT_CONTEXT: &str = "600k"; // 默认上下文窗口
pub const DEFAULT_THINKING: &str = "medium"; // 默认思考强度
pub const DEFAULT_PROVIDER: &str = "deepseek"; // 默认服务商
pub fn thinking_effort_of(raw: &str) -> ThinkingEffort { // 思考档位文本转 goose ThinkingEffort
    match raw.trim().to_lowercase().as_str() { // 归一化比对
        "off" | "disabled" | "none" | "false" => ThinkingEffort::Off, // 关闭思考
        "low" => ThinkingEffort::Low, // 低档
        "high" => ThinkingEffort::High, // 高档
        "max" | "xhigh" => ThinkingEffort::Max, // 极高档
        _ => ThinkingEffort::Medium, // medium / enabled / 默认均按开启中档处理
    }
}
pub fn context_options_of(provider: &str, model: &str) -> Vec<&'static str> { // 按服务商与模型返回可选上下文列表
    if provider.trim().eq_ignore_ascii_case(huawei_maas::PROVIDER_KEY) { return huawei_maas::context_options(model).to_vec(); } // 华为 MaaS 查专属档位
    CONTEXT_OPTIONS.to_vec() // 其他服务商保持三档
}
pub fn thinking_title_of(provider: &str, model: &str) -> &'static str { // 按服务商与模型返回思考参数标题
    if provider.trim().eq_ignore_ascii_case(huawei_maas::PROVIDER_KEY) { return huawei_maas::thinking_title(model); } // 华为区分开关与强度
    "思考强度" // 默认标题
}
pub fn thinking_options_of(provider: &str, model: &str) -> Vec<(&'static str, &'static str)> { // 按服务商与模型返回思考可选项（value, label）
    if provider.trim().eq_ignore_ascii_case(huawei_maas::PROVIDER_KEY) { return huawei_maas::thinking_options(model).to_vec(); } // 华为 MaaS 按模型精确返回
    THINKING_OPTIONS.to_vec() // 其他服务商保留低/中/高
}
pub fn thinking_options_json(provider: &str, model: &str) -> Vec<Value> { // 序列化思考选项供前端直接渲染
    thinking_options_of(provider, model).into_iter().map(|(value, label)| serde_json::json!({"value": value, "label": label})).collect() // 转 {value,label} 对象数组
}
pub fn normalize_context(provider: &str, model: &str, raw: &str) -> String { // 按模型能力校验并归一化上下文档位
    if provider.trim().eq_ignore_ascii_case(huawei_maas::PROVIDER_KEY) { return huawei_maas::normalize_context(model, raw); } // 华为 MaaS 专属校验
    let target = raw.trim(); // 去空白
    if CONTEXT_OPTIONS.contains(&target) { target.to_string() } else { DEFAULT_CONTEXT.to_string() } // 其他回退默认
}
pub fn normalize_thinking(provider: &str, model: &str, raw: &str) -> String { // 按模型能力校验并归一化思考参数
    if provider.trim().eq_ignore_ascii_case(huawei_maas::PROVIDER_KEY) { return huawei_maas::normalize_thinking(model, raw); } // 华为 MaaS 专属校验
    let target = raw.trim().to_lowercase(); // 小写比对
    if THINKING_OPTIONS.iter().any(|(key, _)| *key == target) { target } else { DEFAULT_THINKING.to_string() } // 其他回退默认
}
fn text_of(item: &Value, key: &str) -> String { // 读取字符串字段，缺省空串
    item.get(key).and_then(Value::as_str).map(str::to_string).unwrap_or_default() // 取字符串值
}
fn provider_of(item: &Value) -> String { // 读取服务商名并归一化
    text_of(item, "provider").trim().to_lowercase() // 去空白转小写
}
pub fn preset_display(provider: &str) -> Option<&'static str> { // 内置服务商预设显示名
    match provider.trim().to_lowercase().as_str() { huawei_maas::PROVIDER_KEY => Some(huawei_maas::PROVIDER_DISPLAY), _ => None } // 当前内置华为云 MaaS
}
pub fn preset_base_url(provider: &str) -> Option<&'static str> { // 内置服务商默认 API 地址
    match provider.trim().to_lowercase().as_str() { huawei_maas::PROVIDER_KEY => Some(huawei_maas::DEFAULT_BASE_URL), _ => None } // 返回 MaaS V2 标准地址
}
pub fn default_vision_of(provider: &str, model: &str) -> bool { // 新建模型时的默认视觉能力
    if provider.trim().eq_ignore_ascii_case(huawei_maas::PROVIDER_KEY) { return huawei_maas::default_vision(model); } // 华为 MaaS 查预置元信息
    false // 其他默认不支持
}
pub fn display_of(item: &Value) -> String { // 显示名：优先 llm 项的 display，其次内置显示名，最后回落到 provider
    let display = text_of(item, "display").trim().to_string(); // 配置里的显示名
    if !display.is_empty() { return display; } // 有显示名直接返回
    let provider = provider_of(item); // 取服务商名
    if let Some(name) = preset_display(&provider) { return name.to_string(); } // 命中内置服务商展示名
    if provider.is_empty() { DEFAULT_PROVIDER.to_string() } else { provider } // 兜底返回服务商键或默认值
}
pub fn vision_of(item: &Value) -> bool { // 视觉能力：优先配置项 vision，否则查预置能力
    item.get("vision").and_then(Value::as_bool).unwrap_or_else(|| default_vision_of(&provider_of(item), &text_of(item, "model"))) // 缺省查默认能力
}
pub fn normalize_llm(raw: &Value) -> Vec<Value> { // 把 llm 配置归一化为条目数组
    match raw { // 按配置形态分派
        Value::Object(map) if !map.is_empty() => vec![raw.clone()], // 单对象包成一项
        Value::Array(items) => items.iter().filter(|item| item.is_object()).cloned().collect(), // 数组只保留对象项
        _ => Vec::new(), // 其他形态视为无配置
    }
}
pub fn provider_keys(items: &[Value]) -> Vec<String> { // 全部可用服务商（配置中的服务商 + 内置服务商预设）
    let mut keys: Vec<String> = Vec::new(); // 服务商键集合
    for item in items { // 遍历模型条目
        let provider = provider_of(item); // 取服务商名
        if !provider.is_empty() && !keys.contains(&provider) { keys.push(provider); } // 非空且未出现才加入
    }
    if !keys.contains(&huawei_maas::PROVIDER_KEY.to_string()) { keys.push(huawei_maas::PROVIDER_KEY.to_string()); } // 确保华为 MaaS 始终可选
    keys // 返回去重后的服务商列表
}
pub fn has(items: &[Value], provider: &str) -> bool { // 判断是否受支持的服务商（含配置项与内置预设）
    let target = provider.trim().to_lowercase(); // 归一化目标名
    !target.is_empty() && provider_keys(items).iter().any(|key| key == &target) // 非空且命中
}
fn push_unique_models(models: &mut Vec<Value>, extras: &[String]) { // 向模型数组去重追加模型名
    for name in extras { if !name.is_empty() && !models.iter().any(|v| v.as_str() == Some(name.as_str())) { models.push(Value::String(name.clone())); } } // 仅追加不重复项
}
pub fn providers_json(items: &[Value]) -> Vec<Value> { // 聚合服务商列表，并补齐内置华为 MaaS 预设与主推文本模型
    let mut list: Vec<Value> = Vec::new(); // 聚合结果
    for item in items { // 遍历配置项
        let provider = provider_of(item); // 服务商名
        if provider.is_empty() { continue; } // 无服务商跳过
        let model = text_of(item, "model").trim().to_string(); // 模型名
        let display = display_of(item); // 显示名
        let base_url = text_of(item, "base_url").trim().to_string(); // API 地址
        let resolved_url = if base_url.is_empty() { preset_base_url(&provider).unwrap_or_default().to_string() } else { base_url }; // 补齐默认地址
        match list.iter_mut().find(|entry| entry.get("value").and_then(Value::as_str) == Some(provider.as_str())) { // 查找已聚合的服务商
            Some(entry) => { if let Some(models) = entry.get_mut("models").and_then(Value::as_array_mut) { push_unique_models(models, &[model]); } } // 补全模型名
            None => list.push(serde_json::json!({"value": provider, "display": display, "base_url": resolved_url, "models": if model.is_empty() { Vec::new() } else { vec![model] }})), // 新建服务商条目
        }
    }
    let huawei_models = huawei_maas::model_names(); // 华为 MaaS 主推文本模型
    match list.iter_mut().find(|entry| entry.get("value").and_then(Value::as_str) == Some(huawei_maas::PROVIDER_KEY)) { // 检查是否已有华为项
        Some(entry) => { // 已存在配置时补全 display/base_url 与预置模型候选项
            if entry.get("base_url").and_then(Value::as_str).unwrap_or_default().is_empty() { entry["base_url"] = Value::String(huawei_maas::DEFAULT_BASE_URL.to_string()); } // 补全默认地址
            if let Some(models) = entry.get_mut("models").and_then(Value::as_array_mut) { push_unique_models(models, &huawei_models); } // 合并主推模型选项
        }
        None => list.push(serde_json::json!({"value": huawei_maas::PROVIDER_KEY, "display": huawei_maas::PROVIDER_DISPLAY, "base_url": huawei_maas::DEFAULT_BASE_URL, "models": huawei_models})), // 缺省也注入华为预设
    }
    list // 返回聚合后的服务商列表
}
#[derive(Clone)] // 配置可克隆
pub struct ModelConfig { // 单个模型解析结果
    pub provider: String, // 服务商
    pub base_url: String, // API 地址
    pub api_key: String, // API Key
    pub model: String, // 模型名
    pub enabled: bool, // 是否启用
    pub context: String, // 上下文档位
    pub thinking: String, // 思考强度档位
    pub vision: bool, // 是否支持视觉
}
pub fn default_context_of(provider: &str, model: &str) -> &'static str { // 按服务商与模型返回默认上下文档位
    if provider.trim().eq_ignore_ascii_case(huawei_maas::PROVIDER_KEY) { return huawei_maas::default_context(model); } // 华为查模型默认档位
    DEFAULT_CONTEXT // 其他默认 600k
}
pub fn default_thinking_of(provider: &str, model: &str) -> &'static str { // 按服务商与模型返回默认思考档位
    if provider.trim().eq_ignore_ascii_case(huawei_maas::PROVIDER_KEY) { return huawei_maas::default_thinking(model); } // 华为查模型默认思考
    DEFAULT_THINKING // 其他默认 medium
}
pub fn parse_model(item: &Value) -> ModelConfig { // 解析单条 llm 配置为 ModelConfig
    let provider = provider_of(item); // 服务商名
    let provider = if provider.is_empty() { DEFAULT_PROVIDER.to_string() } else { provider }; // 空则回退默认
    let raw_url = text_of(item, "base_url").trim().to_string(); // 配置里的地址
    let base_url = if raw_url.is_empty() { preset_base_url(&provider).unwrap_or_default().to_string() } else { raw_url }; // 缺省回退预设地址
    let model = text_of(item, "model").trim().to_string(); // 模型名
    let raw_context = item.get("context").and_then(Value::as_str).map(|text| text.trim().to_string()).filter(|text| !text.is_empty()).unwrap_or_else(|| default_context_of(&provider, &model).to_string()); // 上下文档位
    let raw_thinking = item.get("thinking").and_then(Value::as_str).map(|text| text.trim().to_lowercase()).filter(|text| !text.is_empty()).unwrap_or_else(|| default_thinking_of(&provider, &model).to_string()); // 思考强度档位
    let context = normalize_context(&provider, &model, &raw_context); // 按模型校验归一化上下文
    let thinking = normalize_thinking(&provider, &model, &raw_thinking); // 按模型校验归一化思考
    ModelConfig { // 组装模型配置
        provider, // 服务商
        base_url, // API 地址
        api_key: text_of(item, "api_key").trim().to_string(), // API Key
        model, // 模型名
        enabled: item.get("enabled").and_then(Value::as_bool).unwrap_or(true), // 缺省启用
        context, // 合法上下文
        thinking, // 合法思考参数
        vision: vision_of(item), // 视觉能力
    }
}
pub fn active_model(raw: &Value) -> Option<ModelConfig> { // 选择首个启用且配置了模型名的模型
    for item in normalize_llm(raw) { // 遍历归一化条目
        let config = parse_model(&item); // 解析为配置
        if config.enabled && !config.model.is_empty() { return Some(config); } // 命中启用且非空模型
    }
    None // 无可用模型
}
pub fn context_limit_of(context: &str) -> usize { // 上下文档位转具体 token 数
    match context { "200k" => 200_000, "1M" => 1_000_000, _ => 600_000 } // 三档映射，未知按 600k
}
pub fn goose_model_config(config: &ModelConfig) -> GooseModelConfig { // 构造 goose 模型配置
    let effort = thinking_effort_of(&config.thinking); // 统一思考强度映射（含 off/disabled/max）
    let mut limit = context_limit_of(&config.context); // 配置上下文上限
    if config.provider == huawei_maas::PROVIDER_KEY { limit = huawei_maas::clamp_context_limit(&config.model, limit); } // 华为 MaaS 按实测窗口裁剪
    GooseModelConfig::new(&config.model).with_context_limit(Some(limit)).with_vision_support(config.vision).with_default_thinking_effort(Some(effort)) // 注入模型名/上下文/视觉/思考强度
}
pub fn goose_provider(config: &ModelConfig) -> Result<Arc<dyn GooseProvider>, String> { // 按服务商分派构建 goose provider
    if config.provider == huawei_maas::PROVIDER_KEY { return huawei_maas::build_provider(config); } // 华为走 MaaS V2 适配器
    openapi::build_provider(config) // 其余走 OpenAI 兼容适配器
}
pub struct GooseModel { // 供状态机装配：goose 模型配置 + 进程内 provider
    pub config: ModelConfig, // 原始模型配置
    pub model: GooseModelConfig, // goose 模型配置
    pub provider: Arc<dyn GooseProvider>, // goose provider 实例
}
pub fn build_goose_model(raw: &Value) -> Result<GooseModel, String> { // 从 llm 配置构建完整 goose 模型
    let config = active_model(raw).ok_or_else(|| "没有已启用的模型".to_string())?; // 取生效模型，无则报错
    let model = goose_model_config(&config); // 构造 goose 模型配置
    let provider = goose_provider(&config)?; // 构造 provider，失败上抛
    Ok(GooseModel { config, model, provider }) // 返回完整模型
}
