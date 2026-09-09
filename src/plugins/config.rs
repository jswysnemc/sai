use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use sai_plugin_runtime::Capabilities;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Write;

pub(super) use super::management_lock::acquire as mutation_lock;

/// 【插件】【用户配置】独立于 AppConfig 的插件开关、设置和授权。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct PluginConfig {
    pub api_version: u32,
    pub plugins: BTreeMap<String, PluginSetting>,
}

/// 【插件】【实例设置】省略授权时，外部插件没有网络权限。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct PluginSetting {
    pub enabled: bool,
    pub grants: Option<Capabilities>,
    pub settings: Value,
}

impl Default for PluginConfig {
    /// 返回空插件配置，API 版本固定为 1，无参数。
    fn default() -> Self {
        Self {
            api_version: 1,
            plugins: BTreeMap::new(),
        }
    }
}

impl Default for PluginSetting {
    /// 返回禁用且没有授权的默认实例设置，无参数。
    fn default() -> Self {
        Self {
            enabled: false,
            grants: None,
            settings: serde_json::json!({}),
        }
    }
}

/// 【插件】【配置读取】读取独立 JSONC 配置，不创建文件。
/// @param paths Sai 路径
/// @returns 已验证配置；文件不存在时返回缺省配置
pub(crate) fn load_config(paths: &SaiPaths) -> Result<PluginConfig> {
    let path = paths.config_dir.join("plugins.jsonc");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PluginConfig::default())
        }
        Err(error) => return Err(error).context("read plugins.jsonc"),
    };
    if text.len() > 1024 * 1024 {
        bail!("plugin configuration exceeds 1 MiB");
    }
    let config: PluginConfig =
        serde_json::from_reader(json_comments::StripComments::new(text.as_bytes()))
            .context("parse plugins.jsonc")?;
    validate(&config)?;
    Ok(config)
}

/// 【插件】【配置保存】通过同目录临时文件原子替换插件配置。
/// @param paths Sai 路径；config 为完整配置
/// @returns 校验和保存结果
pub(crate) fn save_config(paths: &SaiPaths, config: &PluginConfig) -> Result<()> {
    validate(config)?;
    let bytes = serde_json::to_vec_pretty(config)?;
    if bytes.len() >= 1024 * 1024 {
        bail!("plugin configuration exceeds 1 MiB");
    }
    std::fs::create_dir_all(&paths.config_dir)?;
    let mut file = tempfile::NamedTempFile::new_in(&paths.config_dir)?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.as_file().sync_all()?;
    file.persist(paths.config_dir.join("plugins.jsonc"))
        .context("save plugins.jsonc atomically")?;
    Ok(())
}

/// 【插件】【配置校验】验证版本和每项显式授权。
/// @param config 待校验配置
/// @returns 可加载时成功，未知版本和非法授权返回错误
fn validate(config: &PluginConfig) -> Result<()> {
    if config.api_version != 1 {
        bail!(
            "unsupported plugins.jsonc API version {}",
            config.api_version
        );
    }
    for (id, setting) in &config.plugins {
        if id.is_empty()
            || id.len() > 32
            || !id.as_bytes()[0].is_ascii_lowercase()
            || !id.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte)
            })
        {
            bail!("invalid configured plugin id: {id}");
        }
        if !setting.settings.is_object() {
            bail!("plugin {id} settings must be an object");
        }
        if let Some(grants) = &setting.grants {
            grants.validate()?;
        }
    }
    Ok(())
}
