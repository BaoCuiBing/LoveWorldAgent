use std::sync::Arc; // OpenAI 兼容协议模型构建器：用于 DeepSeek/Kimi/GLM/自定义 OpenAI 兼容服务商
use goose_provider_types::base::Provider as GooseProvider; // goose provider trait
use goose_providers::api_client::{ApiClient, AuthMethod}; // HTTP 客户端与鉴权方式
use goose_providers::openai_compatible::OpenAiCompatibleProvider; // OpenAI 兼容 provider
use super::ModelConfig; // 共享模型配置结构
pub fn build_client(config: &ModelConfig, provider_label: &str) -> Result<ApiClient, String> { // 构造带 TLS 与 Bearer 鉴权的客户端
    let base_url = config.base_url.trim().trim_end_matches('/').to_string(); // 去除尾部斜杠
    if base_url.is_empty() { return Err(format!("{provider_label} 模型 base_url 为空")); } // 空地址报错
    let key = config.api_key.trim(); // 清理空白
    let auth = if key.is_empty() { AuthMethod::NoAuth } else { AuthMethod::BearerToken(key.to_string()) }; // 有 Key 用 Bearer，否则无鉴权
    ApiClient::new_with_tls(base_url, auth, None).map_err(|err| err.to_string()) // 构造 TLS 客户端
}
pub fn build_provider(config: &ModelConfig) -> Result<Arc<dyn GooseProvider>, String> { // 构造 OpenAI 兼容 goose provider
    let client = build_client(config, &config.provider)?; // 创建客户端
    Ok(Arc::new(OpenAiCompatibleProvider::new(config.provider.clone(), client, String::new()))) // 包装为 goose provider
}
