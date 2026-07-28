use agent_client_protocol::schema::v1::{AuthMethod, InitializeResponse};
use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::Value;

/// ACP 握手后可供运行期判断的能力集合。
#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize)]
pub(crate) struct AcpCapabilities {
    pub(crate) load_session: bool,
    pub(crate) list_sessions: bool,
    pub(crate) delete_session: bool,
    pub(crate) resume_session: bool,
    pub(crate) close_session: bool,
    pub(crate) additional_directories: bool,
    pub(crate) mcp_http: bool,
    pub(crate) mcp_sse: bool,
    pub(crate) prompt_image: bool,
    pub(crate) prompt_audio: bool,
    pub(crate) embedded_context: bool,
    pub(crate) logout: bool,
    pub(crate) sai_context_compaction: bool,
    pub(crate) sai_memory: bool,
    pub(crate) sai_goal_continuation: bool,
    pub(crate) sai_subagents: bool,
}

/// 查询指定外部内核最近一次握手能力。
///
/// 参数:
/// - `engine`: Sai 外部内核稳定名称
///
/// 返回:
/// - 本进程已完成握手时返回能力集合
pub(crate) fn current(engine: &str) -> Option<AcpCapabilities> {
    super::runtime_state::current(engine).map(|state| state.capabilities)
}

/// ACP 初始化结果。
#[derive(Debug, Clone)]
pub(crate) struct InitializedAgent {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) capabilities: AcpCapabilities,
    pub(crate) auth_methods: Vec<AuthMethod>,
    pub(crate) native_equivalents: Value,
}

/// 使用官方 SDK 解析握手响应并提取运行期能力。
///
/// 参数:
/// - `value`: `initialize` 响应结果
///
/// 返回:
/// - agent 信息与能力集合
pub(crate) fn parse_initialize_response(value: Value) -> Result<InitializedAgent> {
    let response: InitializeResponse = super::sdk::from_value(value, "initialize response")?;
    if response.protocol_version.as_u16() != 1 {
        bail!(
            "ACP agent speaks protocol version {}, sai supports 1",
            response.protocol_version.as_u16()
        );
    }
    let sai_metadata = response
        .meta
        .as_ref()
        .and_then(|meta| meta.get("_sai"))
        .cloned();
    let sai_features = sai_metadata
        .as_ref()
        .and_then(|sai| sai.get("capabilities"));
    let native_equivalents = sai_metadata
        .as_ref()
        .and_then(|sai| sai.get("native_equivalents"))
        .cloned()
        .unwrap_or(Value::Null);
    let sai_feature = |name: &str| {
        sai_features
            .and_then(|features| features.get(name))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    let info = response.agent_info;
    let name = info
        .as_ref()
        .map(|info| info.title.as_deref().unwrap_or(&info.name).to_string())
        .unwrap_or_else(|| "ACP agent".to_string());
    let version = info
        .as_ref()
        .map(|info| info.version.clone())
        .unwrap_or_else(|| "-".to_string());
    let auth_methods = response.auth_methods;
    let caps = response.agent_capabilities;
    let session = &caps.session_capabilities;
    let prompt = &caps.prompt_capabilities;
    Ok(InitializedAgent {
        name,
        version,
        capabilities: AcpCapabilities {
            load_session: caps.load_session,
            list_sessions: session.list.is_some(),
            delete_session: session.delete.is_some(),
            resume_session: session.resume.is_some(),
            close_session: session.close.is_some(),
            additional_directories: session.additional_directories.is_some(),
            mcp_http: caps.mcp_capabilities.http,
            mcp_sse: caps.mcp_capabilities.sse,
            prompt_image: prompt.image,
            prompt_audio: prompt.audio,
            embedded_context: prompt.embedded_context,
            logout: caps.auth.logout.is_some(),
            sai_context_compaction: sai_feature("context_compaction"),
            sai_memory: sai_feature("memory"),
            sai_goal_continuation: sai_feature("goal_continuation"),
            sai_subagents: sai_feature("subagents"),
        },
        auth_methods,
        native_equivalents,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_initialize_response;

    /// 会话能力必须来自握手，而不是按“外部内核”统一禁用。
    #[test]
    fn parses_session_lifecycle_capabilities() {
        let parsed = parse_initialize_response(serde_json::json!({
            "protocolVersion": 1,
            "agentInfo": { "name": "agent", "version": "1.2.3" },
            "agentCapabilities": {
                "loadSession": true,
                "sessionCapabilities": {
                    "list": {}, "delete": {}, "resume": {}, "close": {},
                    "additionalDirectories": {}
                },
                "mcpCapabilities": { "http": true, "sse": false }
            }
        }))
        .unwrap();
        assert!(parsed.capabilities.load_session);
        assert!(parsed.capabilities.list_sessions);
        assert!(parsed.capabilities.delete_session);
        assert!(parsed.capabilities.resume_session);
        assert!(parsed.capabilities.close_session);
        assert!(parsed.capabilities.additional_directories);
        assert!(parsed.capabilities.mcp_http);
    }

    /// 提示内容与认证能力也必须按握手结果记录。
    #[test]
    fn parses_prompt_and_auth_capabilities() {
        let parsed = parse_initialize_response(serde_json::json!({
            "protocolVersion": 1,
            "agentCapabilities": {
                "promptCapabilities": {
                    "image": true,
                    "audio": true,
                    "embeddedContext": true
                },
                "auth": { "logout": {} }
            },
            "authMethods": [{ "id": "login", "name": "Login" }]
        }))
        .unwrap();

        assert!(parsed.capabilities.prompt_image);
        assert!(parsed.capabilities.prompt_audio);
        assert!(parsed.capabilities.embedded_context);
        assert!(parsed.capabilities.logout);
        assert_eq!(parsed.auth_methods[0].id().to_string(), "login");
    }

    /// Sai Sidecar 只能声明已经真实接通的宿主集成能力。
    #[test]
    fn parses_sai_integration_capabilities() {
        let parsed = parse_initialize_response(serde_json::json!({
            "protocolVersion": 1,
            "agentCapabilities": {},
            "_meta": {
                "_sai": {
                    "capabilities": {
                        "context_compaction": true,
                        "memory": true,
                        "goal_continuation": true,
                        "subagents": true
                    },
                    "native_equivalents": {
                        "context_compaction": "codex",
                        "subagents": "codex"
                    }
                }
            }
        }))
        .unwrap();

        assert!(parsed.capabilities.sai_context_compaction);
        assert!(parsed.capabilities.sai_memory);
        assert!(parsed.capabilities.sai_goal_continuation);
        assert!(parsed.capabilities.sai_subagents);
        assert_eq!(parsed.native_equivalents["context_compaction"], "codex");
    }
}
