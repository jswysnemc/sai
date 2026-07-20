use super::client::McpToolInfo;
use crate::config::McpServerConfig;
use crate::paths::SaiPaths;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;

const CACHE_VERSION: u32 = 1;
const CACHE_FILE: &str = "mcp-tools.json";

/// MCP 工具元数据缓存。
#[derive(Default, Serialize, Deserialize)]
struct McpToolCache {
    version: u32,
    servers: Vec<CachedServerTools>,
}

/// 单个 MCP 服务对应的缓存工具集合。
#[derive(Serialize, Deserialize)]
struct CachedServerTools {
    server_id: String,
    config_fingerprint: String,
    tools: Vec<McpToolInfo>,
}

/// 读取与当前 MCP 配置匹配的工具元数据。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `servers`: 当前 MCP 服务配置
///
/// 返回:
/// - 配置指纹仍有效的已启用服务工具
pub(super) fn load(paths: &SaiPaths, servers: &[McpServerConfig]) -> Vec<McpToolInfo> {
    let Ok(content) = std::fs::read(cache_path(paths)) else {
        return Vec::new();
    };
    let Ok(cache) = serde_json::from_slice::<McpToolCache>(&content) else {
        return Vec::new();
    };
    if cache.version != CACHE_VERSION {
        return Vec::new();
    }
    let fingerprints = servers
        .iter()
        .filter(|server| server.enabled)
        .map(|server| (server.id.as_str(), fingerprint(server)))
        .collect::<BTreeMap<_, _>>();
    cache
        .servers
        .into_iter()
        .filter(|entry| {
            fingerprints
                .get(entry.server_id.as_str())
                .is_some_and(|fingerprint| fingerprint == &entry.config_fingerprint)
        })
        .flat_map(|entry| entry.tools)
        .collect()
}

/// 原子更新单个 MCP 服务的工具元数据缓存。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `server`: 已完成工具发现的服务配置
/// - `tools`: 服务当前工具元数据
///
/// 返回:
/// - 缓存写入结果
pub(super) fn store_server(
    paths: &SaiPaths,
    server: &McpServerConfig,
    tools: &[McpToolInfo],
) -> Result<()> {
    let mut cache = read_cache(paths);
    cache.version = CACHE_VERSION;
    cache.servers.retain(|entry| entry.server_id != server.id);
    cache.servers.push(CachedServerTools {
        server_id: server.id.clone(),
        config_fingerprint: fingerprint(server),
        tools: tools.to_vec(),
    });
    cache
        .servers
        .sort_by(|left, right| left.server_id.cmp(&right.server_id));
    std::fs::create_dir_all(&paths.cache_dir)?;
    let path = cache_path(paths);
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, serde_json::to_vec_pretty(&cache)?)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

/// 读取缓存文件，损坏或版本不匹配时返回空缓存。
///
/// 参数:
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 可继续更新的缓存结构
fn read_cache(paths: &SaiPaths) -> McpToolCache {
    let Ok(content) = std::fs::read(cache_path(paths)) else {
        return McpToolCache::default();
    };
    serde_json::from_slice::<McpToolCache>(&content)
        .ok()
        .filter(|cache| cache.version == CACHE_VERSION)
        .unwrap_or_default()
}

/// 计算不暴露环境变量和请求头内容的稳定配置指纹。
///
/// 参数:
/// - `server`: MCP 服务配置
///
/// 返回:
/// - 十六进制 SHA-256 指纹
fn fingerprint(server: &McpServerConfig) -> String {
    let mut hasher = Sha256::new();
    hasher.update(server.id.as_bytes());
    hasher.update([server.enabled as u8]);
    hasher.update(server.transport.as_bytes());
    hasher.update(server.command.as_bytes());
    for argument in &server.args {
        hasher.update([0]);
        hasher.update(argument.as_bytes());
    }
    for (key, value) in server.env.iter().collect::<BTreeMap<_, _>>() {
        hasher.update(key.as_bytes());
        hasher.update([0]);
        hasher.update(value.as_bytes());
    }
    hasher.update(server.cwd.as_deref().unwrap_or_default().as_bytes());
    hasher.update(server.url.as_deref().unwrap_or_default().as_bytes());
    hasher.update(server.message_url.as_deref().unwrap_or_default().as_bytes());
    for (key, value) in server.headers.iter().collect::<BTreeMap<_, _>>() {
        hasher.update(key.as_bytes());
        hasher.update([0]);
        hasher.update(value.as_bytes());
    }
    hasher.update(server.timeout_ms.unwrap_or_default().to_le_bytes());
    format!("{:x}", hasher.finalize())
}

/// 返回 MCP 工具缓存文件路径。
///
/// 参数:
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 缓存文件路径
fn cache_path(paths: &SaiPaths) -> PathBuf {
    paths.cache_dir.join(CACHE_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造缓存测试使用的 Sai 路径。
    ///
    /// 参数:
    /// - `root`: 临时目录根路径
    ///
    /// 返回:
    /// - 指向临时目录的路径集合
    fn test_paths(root: &std::path::Path) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config.jsonc"),
            secrets_file: root.join("secrets.jsonc"),
            skills_dir: root.join("skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("sai.fish"),
            bash_hook_file: root.join("bash-hook.sh"),
            zsh_hook_file: root.join("zsh-hook.zsh"),
            powershell_hook_file: root.join("powershell-hook.ps1"),
        }
    }

    /// 构造缓存测试使用的 MCP 服务配置。
    ///
    /// 参数:
    /// - `command`: 服务启动命令
    ///
    /// 返回:
    /// - 已启用的 stdio 服务配置
    fn server(command: &str) -> McpServerConfig {
        McpServerConfig {
            id: "files".to_string(),
            enabled: true,
            transport: "stdio".to_string(),
            command: command.to_string(),
            args: Vec::new(),
            env: Default::default(),
            cwd: None,
            url: None,
            message_url: None,
            headers: Default::default(),
            timeout_ms: None,
        }
    }

    #[test]
    fn cache_round_trip_requires_matching_server_config() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let server = server("mcp-files");
        let tools = vec![McpToolInfo {
            server_id: server.id.clone(),
            name: "read".to_string(),
            description: "Read a file".to_string(),
            input_schema: serde_json::json!({"type":"object"}),
        }];

        store_server(&paths, &server, &tools).unwrap();

        assert_eq!(load(&paths, std::slice::from_ref(&server)), tools);
        assert!(load(&paths, &[self::server("changed-command")]).is_empty());
    }

    #[test]
    fn cache_file_does_not_expose_server_secrets() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let mut server = server("mcp-files");
        server
            .env
            .insert("TOKEN".to_string(), "secret-env".to_string());
        server
            .headers
            .insert("Authorization".to_string(), "secret-header".to_string());

        store_server(&paths, &server, &[]).unwrap();

        let content = std::fs::read_to_string(cache_path(&paths)).unwrap();
        assert!(!content.contains("secret-env"));
        assert!(!content.contains("secret-header"));
    }
}
