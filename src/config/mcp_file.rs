use super::model::{McpConfig, McpServerConfig};
use super::secrets::{set_private_permissions, write_private_file};
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use serde_json::{Map, Value};

/// 从独立 `mcp.jsonc` 加载 MCP 配置；不存在时返回默认。
///
/// 若独立文件尚不存在，而 `config.jsonc` 内仍有 legacy `mcp` 段，
/// 会自动迁移写出 `mcp.jsonc` 并返回迁移结果。
pub fn load_mcp_config(paths: &SaiPaths) -> Result<McpConfig> {
    let file = paths.mcp_config_file();
    if file.exists() {
        // 1. MCP env 和 headers 可能包含凭据，读取已有文件前先收紧权限
        set_private_permissions(&file)?;
        let raw = std::fs::read_to_string(&file)
            .with_context(|| format!("failed to read {}", file.display()))?;
        let stripped = json_comments::StripComments::new(raw.as_bytes());
        let value: Value = serde_json::from_reader(stripped)
            .with_context(|| format!("invalid JSONC in {}", file.display()))?;
        let config = parse_mcp_config_value(value)
            .with_context(|| format!("invalid MCP configuration in {}", file.display()))?;
        validate_mcp_config(&config)?;
        return Ok(config);
    }
    Ok(McpConfig::default())
}

/// 保存 MCP 配置到独立文件。
pub fn save_mcp_config(paths: &SaiPaths, config: &McpConfig) -> Result<()> {
    validate_mcp_config(config)?;
    paths.create_dirs()?;
    let file = paths.mcp_config_file();
    let raw = serde_json::to_string_pretty(config)?;
    write_private_file(&file, format!("{raw}\n").as_bytes())
        .with_context(|| format!("failed to write {}", file.display()))?;
    Ok(())
}

/// 初始化独立 MCP 配置文件。
///
/// 优先迁移 `config.jsonc` 内的 legacy `mcp` 段；否则写默认空配置。
pub fn init_mcp_config_file(paths: &SaiPaths, legacy: Option<&McpConfig>) -> Result<()> {
    let file = paths.mcp_config_file();
    if file.exists() {
        set_private_permissions(&file)?;
        return Ok(());
    }
    let config = legacy.cloned().unwrap_or_default();
    save_mcp_config(paths, &config)
}

/// 从主配置迁移 legacy MCP 配置，并确保独立文件权限正确。
///
/// 参数:
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 独立 MCP 配置初始化结果
pub(super) fn init_mcp_config_from_main(paths: &SaiPaths) -> Result<()> {
    let legacy = (!paths.mcp_config_file().exists())
        .then(|| read_legacy_mcp_from_main_config(paths))
        .flatten();
    init_mcp_config_file(paths, legacy.as_ref())
}

/// 仅解析主配置中的 legacy `mcp` 段，不触发完整配置校验。
fn read_legacy_mcp_from_main_config(paths: &SaiPaths) -> Option<McpConfig> {
    if !paths.config_file.exists() {
        return None;
    }
    let raw = std::fs::read_to_string(&paths.config_file).ok()?;
    let stripped = json_comments::StripComments::new(raw.as_bytes());
    let value: serde_json::Value = serde_json::from_reader(stripped).ok()?;
    parse_mcp_config_value(value.get("mcp")?.clone()).ok()
}

/// 解析 Sai 数组格式或标准 `mcpServers` 对象格式。
///
/// 标准格式使用对象键标识服务，因此服务对象可以省略 `id`；若对象内仍提供
/// 非空 `id`，显式值优先，保持现有配置兼容性。
///
/// 参数:
/// - `value`: MCP 配置 JSON 值
///
/// 返回:
/// - 归一化后的内部 MCP 配置
pub fn parse_mcp_config_value(value: Value) -> Result<McpConfig> {
    let root = value
        .as_object()
        .context("MCP configuration must be a JSON object")?;

    // 1. Sai 原生数组格式保持原有反序列化行为
    if root.get("servers").is_some_and(Value::is_array) {
        return serde_json::from_value(value).context("invalid MCP servers array");
    }

    // 2. 同时兼容标准 mcpServers 映射和 servers 映射
    let server_map = root
        .get("mcpServers")
        .or_else(|| root.get("servers"))
        .filter(|servers| !servers.is_null());
    let Some(server_map) = server_map else {
        return serde_json::from_value(value).context("invalid MCP configuration");
    };
    let server_map = server_map
        .as_object()
        .context("mcpServers must be an object and servers must be an array or object")?;
    let enabled = root.get("enabled").and_then(Value::as_bool).unwrap_or(true);
    let servers = server_map
        .iter()
        .map(|(map_id, server)| normalize_mcp_server(map_id, server))
        .collect::<Result<Vec<_>>>()?;
    Ok(McpConfig { enabled, servers })
}

/// 将标准映射中的单个 MCP 服务转换为内部结构。
///
/// 参数:
/// - `map_id`: 标准对象中的服务键名
/// - `value`: 服务配置 JSON 值
///
/// 返回:
/// - 带稳定 ID 和传输类型的服务配置
fn normalize_mcp_server(map_id: &str, value: &Value) -> Result<McpServerConfig> {
    let mut server = value
        .as_object()
        .cloned()
        .with_context(|| format!("mcp server {map_id} must be a JSON object"))?;

    // 1. 缺省 ID 使用标准对象键，显式非空 ID 保持优先
    let explicit_id = server
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty());
    if explicit_id.is_none() {
        server.insert("id".to_string(), Value::String(map_id.to_string()));
    }

    // 2. 标准 disabled 字段转换为内部 enabled 字段
    if !server.contains_key("enabled") {
        if let Some(disabled) = server.get("disabled").and_then(Value::as_bool) {
            server.insert("enabled".to_string(), Value::Bool(!disabled));
        }
    }

    // 3. 标准 type 与字段特征转换为内部 transport
    if !server
        .get("transport")
        .and_then(Value::as_str)
        .is_some_and(|transport| !transport.trim().is_empty())
    {
        let transport = infer_mcp_transport(&server);
        server.insert("transport".to_string(), Value::String(transport));
    }

    // 4. 兼容常见 camelCase 字段
    copy_alias(&mut server, "messageUrl", "message_url");
    copy_alias(&mut server, "timeoutMs", "timeout_ms");

    serde_json::from_value(Value::Object(server))
        .with_context(|| format!("invalid mcp server configuration: {map_id}"))
}

/// 根据标准 type、命令或 URL 推断 MCP 传输类型。
///
/// 参数:
/// - `server`: 服务配置对象
///
/// 返回:
/// - `stdio`、`http` 或 `sse`
fn infer_mcp_transport(server: &Map<String, Value>) -> String {
    if let Some(standard_type) = server.get("type").and_then(Value::as_str) {
        return match standard_type.trim().to_ascii_lowercase().as_str() {
            "streamable-http" | "streamable_http" => "http".to_string(),
            "stdio" | "http" | "sse" => standard_type.trim().to_ascii_lowercase(),
            _ => standard_type.trim().to_ascii_lowercase(),
        };
    }
    if server
        .get("url")
        .and_then(Value::as_str)
        .is_some_and(|url| !url.trim().is_empty())
    {
        "http".to_string()
    } else {
        "stdio".to_string()
    }
}

/// 将标准字段别名复制到内部字段名，内部字段已存在时不覆盖。
///
/// 参数:
/// - `server`: 可变服务配置对象
/// - `source`: 标准字段名
/// - `target`: 内部字段名
///
/// 返回:
/// - 无
fn copy_alias(server: &mut Map<String, Value>, source: &str, target: &str) {
    if server.contains_key(target) {
        return;
    }
    if let Some(value) = server.get(source).cloned() {
        server.insert(target.to_string(), value);
    }
}

/// 校验 MCP 配置合法性。
pub fn validate_mcp_config(config: &McpConfig) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    for server in &config.servers {
        validate_mcp_server(server)?;
        if !seen.insert(server.id.clone()) {
            bail!("duplicate mcp server id: {}", server.id);
        }
    }
    Ok(())
}

fn validate_mcp_server(server: &McpServerConfig) -> Result<()> {
    if server.id.trim().is_empty() {
        bail!("mcp server id cannot be empty");
    }
    if server
        .id
        .chars()
        .any(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
    {
        bail!(
            "mcp server id may only contain letters, digits, '_' and '-': {}",
            server.id
        );
    }
    let transport = server.transport.trim().to_ascii_lowercase();
    match transport.as_str() {
        "stdio" => {
            if server.command.trim().is_empty() && server.enabled {
                // 允许草稿保存，但启用时建议有 command；保持宽松以便 UI 逐步填写
            }
        }
        "http" | "sse" => {
            if server.enabled && server.url.as_deref().unwrap_or("").trim().is_empty() {
                bail!(
                    "mcp server {} ({transport}) requires url when enabled",
                    server.id
                );
            }
        }
        other if !other.is_empty() => {
            bail!("unsupported mcp transport for {}: {other}", server.id);
        }
        _ => {}
    }
    if let Some(timeout) = server.timeout_ms {
        if !(100..=300_000).contains(&timeout) {
            bail!(
                "mcp server {} timeout_ms must be between 100 and 300000",
                server.id
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::SaiPaths;
    use std::path::PathBuf;

    fn test_paths(root: PathBuf) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.jsonc"),
            skills_dir: root.join("config/skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish/sai.fish"),
            bash_hook_file: root.join("shell/bash-hook.sh"),
            zsh_hook_file: root.join("shell/zsh-hook.zsh"),
            powershell_hook_file: root.join("shell/powershell-hook.ps1"),
        }
    }

    #[test]
    fn saves_and_loads_mcp_config() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let config = McpConfig {
            enabled: true,
            servers: vec![McpServerConfig {
                id: "fs".into(),
                enabled: true,
                transport: "stdio".into(),
                command: "npx".into(),
                args: vec![
                    "-y".into(),
                    "@modelcontextprotocol/server-filesystem".into(),
                    ".".into(),
                ],
                env: Default::default(),
                cwd: None,
                url: None,
                message_url: None,
                headers: Default::default(),
                timeout_ms: Some(30_000),
            }],
        };
        save_mcp_config(&paths, &config).unwrap();
        let loaded = load_mcp_config(&paths).unwrap();
        assert_eq!(loaded, config);
        assert!(paths.mcp_config_file().exists());
    }

    /// 【MCP】【标准配置】验证 mcpServers 对象键可作为缺省服务 ID。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn loads_standard_mcp_servers_without_explicit_ids() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        std::fs::create_dir_all(&paths.config_dir).unwrap();
        std::fs::write(
            paths.mcp_config_file(),
            r#"{
              "mcpServers": {
                "filesystem": {
                  "command": "npx",
                  "args": ["-y", "@modelcontextprotocol/server-filesystem", "."]
                }
              }
            }"#,
        )
        .unwrap();

        let config = load_mcp_config(&paths).unwrap();

        assert!(config.enabled);
        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.servers[0].id, "filesystem");
        assert_eq!(config.servers[0].transport, "stdio");
        assert_eq!(config.servers[0].command, "npx");
    }

    /// 【MCP】【兼容配置】验证显式 ID 优先于标准对象键并保留传输类型。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn explicit_server_id_takes_precedence_over_map_key() {
        let config = parse_mcp_config_value(serde_json::json!({
            "mcpServers": {
                "remote-alias": {
                    "id": "remote-main",
                    "type": "sse",
                    "url": "https://example.com/sse",
                    "disabled": true
                }
            }
        }))
        .unwrap();

        assert_eq!(config.servers[0].id, "remote-main");
        assert_eq!(config.servers[0].transport, "sse");
        assert!(!config.servers[0].enabled);
    }

    #[cfg(unix)]
    #[test]
    fn saves_mcp_config_with_private_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let config = McpConfig::default();
        save_mcp_config(&paths, &config).unwrap();
        let created_mode = std::fs::metadata(paths.mcp_config_file())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(created_mode, 0o600);
        std::fs::set_permissions(
            paths.mcp_config_file(),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();

        let _ = load_mcp_config(&paths).unwrap();

        let mode = std::fs::metadata(paths.mcp_config_file())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
}
