use agent_client_protocol::schema::v1::{
    BooleanConfigOptionCapabilities, ClientCapabilities, ClientSessionCapabilities,
    ElicitationCapabilities, ElicitationFormCapabilities, FileSystemCapabilities, Implementation,
    InitializeRequest, SessionConfigOptionsCapabilities,
};
use agent_client_protocol::schema::ProtocolVersion;
use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

/// 使用官方 ACP SDK 构造初始化参数。
///
/// 参数:
/// - `client_name`: 握手使用的客户端名称
///
/// 返回:
/// - 可交给现有传输层发送的标准 ACP 参数
pub(crate) fn initialize_params(client_name: &str) -> Result<Value> {
    let fs = FileSystemCapabilities::new()
        .read_text_file(true)
        .write_text_file(true);
    let config_options =
        SessionConfigOptionsCapabilities::new().boolean(BooleanConfigOptionCapabilities::new());
    let session = ClientSessionCapabilities::new().config_options(config_options);
    let elicitation = ElicitationCapabilities::new().form(ElicitationFormCapabilities::new());
    let capabilities = ClientCapabilities::new()
        .fs(fs)
        .terminal(true)
        .session(session)
        .elicitation(elicitation);
    let request = InitializeRequest::new(ProtocolVersion::V1)
        .client_info(Implementation::new(client_name, env!("CARGO_PKG_VERSION")))
        .client_capabilities(capabilities);
    to_value(&request)
}

/// 使用官方 ACP SDK 序列化请求参数。
///
/// 参数:
/// - `params`: SDK 协议类型
///
/// 返回:
/// - JSON-RPC `params` 值
pub(crate) fn to_value<T: Serialize>(params: &T) -> Result<Value> {
    serde_json::to_value(params).context("failed to serialize ACP SDK request")
}

/// 使用官方 ACP SDK 解析响应或通知参数。
///
/// 参数:
/// - `value`: 对端返回的 JSON 值
/// - `operation`: 用于错误上下文的协议动作
///
/// 返回:
/// - SDK 协议类型
pub(crate) fn from_value<T: DeserializeOwned>(value: Value, operation: &str) -> Result<T> {
    serde_json::from_value(value)
        .with_context(|| format!("failed to parse ACP {operation} with the official SDK"))
}

#[cfg(test)]
mod tests {
    use super::initialize_params;

    /// 初始化必须声明布尔配置和结构化提问能力。
    #[test]
    fn initialization_advertises_extended_client_capabilities() {
        let value = initialize_params("sai").unwrap();
        assert_eq!(value["protocolVersion"], 1);
        assert!(value["clientCapabilities"]["session"]["configOptions"]["boolean"].is_object());
        assert!(value["clientCapabilities"]["elicitation"]["form"].is_object());
    }
}
