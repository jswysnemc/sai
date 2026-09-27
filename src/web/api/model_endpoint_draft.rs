use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::services::config_service::SECRET_SENTINEL;
use crate::config::{ModelEndpointConfig, ModelEndpointKind};

/// 【模型接入】【草稿还原】把浏览器草稿中的脱敏密钥换回服务端保存的真实值。
///
/// 设置页未保存时也要能测试接入，因此以草稿为准，只替换仍为占位符的密钥。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `submitted`: 浏览器提交的接入草稿
/// - `kind`: 接口要求的接入类型
///
/// 返回:
/// - 可直接请求的接入；类型不符时返回请求错误
pub(super) fn restore_endpoint_secrets(
    state: &WebAppState,
    mut submitted: ModelEndpointConfig,
    kind: ModelEndpointKind,
) -> WebResult<ModelEndpointConfig> {
    if submitted.kind != kind {
        return Err(WebError::bad_request("model endpoint kind does not match this request"));
    }
    // 1. 读取已保存的同 id 接入
    let config = crate::config::AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    let current = config
        .model_endpoints
        .iter()
        .find(|item| item.id == submitted.id)
        .cloned();
    // 2. 单值密钥占位符还原
    if submitted.api_key == SECRET_SENTINEL {
        submitted.api_key = current
            .as_ref()
            .map(|item| item.api_key.clone())
            .unwrap_or_default();
    }
    // 3. 多密钥按 id 还原
    if let Some(current) = current {
        for key in &mut submitted.api_keys {
            if key.api_key == SECRET_SENTINEL {
                if let Some(previous) = current.api_keys.iter().find(|item| item.id == key.id) {
                    key.api_key = previous.api_key.clone();
                }
            }
        }
    }
    Ok(submitted)
}
