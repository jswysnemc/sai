use super::model::{McpConfig, McpServerConfig};
use super::secrets::{set_private_permissions, write_private_file};
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};

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
        let config: McpConfig = serde_json::from_reader(stripped)
            .with_context(|| format!("invalid JSONC in {}", file.display()))?;
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
    serde_json::from_value(value.get("mcp")?.clone()).ok()
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
