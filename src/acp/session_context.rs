use agent_client_protocol::schema::v1::{
    EnvVariable, HttpHeader, McpServer, McpServerHttp, McpServerSse, McpServerStdio, Meta,
};
use anyhow::Result;
use std::path::PathBuf;

/// 传给 ACP 会话创建、恢复和续接方法的工作区上下文。
#[derive(Debug, Clone, Default)]
pub(crate) struct AcpSessionContext {
    pub(crate) mcp_servers: Vec<McpServer>,
    pub(crate) additional_directories: Vec<PathBuf>,
    pub(crate) meta: Option<Meta>,
}

/// 从 Sai 配置构造标准 ACP 会话上下文。
///
/// 参数:
/// - `governance`: 当前外部内核治理上下文
/// - `capabilities`: 握手协商出的 agent 能力
///
/// 返回:
/// - MCP、附加目录与 Sai Skills 扩展元数据
pub(crate) fn build(
    governance: &super::governance::AcpGovernance,
    capabilities: &super::capabilities::AcpCapabilities,
) -> Result<AcpSessionContext> {
    let config = governance.config();
    let mcp_servers = if config.mcp.enabled {
        config
            .mcp
            .servers
            .iter()
            .filter(|server| server.enabled)
            .filter_map(|server| convert_mcp_server(server, capabilities))
            .collect()
    } else {
        Vec::new()
    };
    let additional_directories = if capabilities.additional_directories {
        config.agent.acp.additional_directories.clone()
    } else {
        Vec::new()
    };
    let meta = governance.paths().and_then(|paths| {
        crate::tools::skills_prompt(config, paths)
            .ok()
            .filter(|skills| !skills.trim().is_empty())
            .map(|skills| {
                let mut sai = serde_json::Map::new();
                sai.insert("skills".to_string(), serde_json::Value::String(skills));
                let mut meta = Meta::new();
                meta.insert("_sai".to_string(), serde_json::Value::Object(sai));
                meta
            })
    });
    Ok(AcpSessionContext {
        mcp_servers,
        additional_directories,
        meta,
    })
}

/// 将 Sai MCP 配置转换为官方 ACP SDK 类型。
///
/// 参数:
/// - `server`: Sai MCP 服务配置
/// - `capabilities`: agent 支持的 MCP 传输
///
/// 返回:
/// - agent 支持时返回标准 ACP MCP 配置
fn convert_mcp_server(
    server: &crate::config::McpServerConfig,
    capabilities: &super::capabilities::AcpCapabilities,
) -> Option<McpServer> {
    let headers = || {
        server
            .headers
            .iter()
            .map(|(name, value)| HttpHeader::new(name, value))
            .collect::<Vec<_>>()
    };
    match server.transport.as_str() {
        "http" if capabilities.mcp_http => Some(McpServer::Http(
            McpServerHttp::new(&server.id, server.url.as_deref()?).headers(headers()),
        )),
        "sse" if capabilities.mcp_sse => Some(McpServer::Sse(
            McpServerSse::new(&server.id, server.url.as_deref()?).headers(headers()),
        )),
        "stdio" => {
            let env = server
                .env
                .iter()
                .map(|(name, value)| EnvVariable::new(name, value))
                .collect();
            Some(McpServer::Stdio(
                McpServerStdio::new(&server.id, &server.command)
                    .args(server.args.clone())
                    .env(env),
            ))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::convert_mcp_server;
    use crate::acp::capabilities::AcpCapabilities;
    use crate::config::McpServerConfig;

    /// HTTP MCP 仅在 agent 声明支持后传入会话。
    #[test]
    fn filters_mcp_transports_by_handshake_capability() {
        let server = McpServerConfig {
            id: "docs".to_string(),
            enabled: true,
            transport: "http".to_string(),
            command: String::new(),
            args: Vec::new(),
            env: Default::default(),
            cwd: None,
            url: Some("https://example.test/mcp".to_string()),
            message_url: None,
            headers: Default::default(),
            timeout_ms: None,
        };
        assert!(convert_mcp_server(&server, &AcpCapabilities::default()).is_none());
        let supported = AcpCapabilities {
            mcp_http: true,
            ..Default::default()
        };
        assert!(convert_mcp_server(&server, &supported).is_some());
    }
}
