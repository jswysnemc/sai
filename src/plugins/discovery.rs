use super::bundled;
use super::config::{load_config, PluginSetting};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use sai_plugin_runtime::{Capabilities, PluginPackage};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::PathBuf;

/// 【插件】【来源】只接受随程序发布的包和用户显式安装的包。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "path")]
pub(crate) enum PluginSource {
    Bundled,
    Installed(PathBuf),
}

/// 【插件】【发现结果】源码、配置和授权共同组成一次不可变加载输入。
#[derive(Clone, Debug)]
pub(crate) struct PluginDescriptor {
    pub package: PluginPackage,
    pub source: PluginSource,
    pub setting: PluginSetting,
}

/// 【插件】【诊断】单个坏包不阻止其他包进入注册事务。
#[derive(Clone, Debug, Serialize)]
pub(crate) struct PluginDiagnostic {
    pub source: String,
    pub error: String,
}

#[derive(Default)]
pub(crate) struct Discovery {
    pub plugins: Vec<PluginDescriptor>,
    pub diagnostics: Vec<PluginDiagnostic>,
}

impl PluginDescriptor {
    /// 【插件】【授权合并】内置包沿用发布时授权，外部包缺省没有授权。
    /// @returns 用户授权；运行时还会与清单声明求交集
    pub fn grants(&self) -> Capabilities {
        self.setting
            .grants
            .clone()
            .unwrap_or_else(|| match self.source {
                PluginSource::Bundled => self.package.manifest.capabilities.clone(),
                PluginSource::Installed(_) => Capabilities::default(),
            })
    }

    /// 【插件】【版本比较】为源码、设置和授权计算稳定标识。
    /// @returns 用于同一会话替换工具表时复用实例的内容摘要
    pub fn revision(&self) -> Result<String> {
        let bytes = serde_json::to_vec(&(
            &self.package.manifest,
            self.package.sources(),
            &self.setting.settings,
            self.grants(),
        ))?;
        Ok(blake3::hash(&bytes).to_hex().to_string())
    }

    /// 【插件】【工具名称】为外部工具添加插件命名空间并验证供应商长度限制。
    /// @param local 包内工具名
    /// @returns 对外稳定名称；内置迁移包保留既有工具名
    pub fn tool_name(&self, local: &str) -> Result<String> {
        public_tool_name(
            &self.package.manifest.id,
            local,
            matches!(self.source, PluginSource::Bundled),
        )
    }
}

/// 【插件】【工具名称】集中约束所有对模型公开的插件工具名。
/// @param id 插件标识；local 为包内名称；bundled 表示保留兼容名称
/// @returns 不超过 64 字节的工具名，避免截断后发生冲突
pub(super) fn public_tool_name(id: &str, local: &str, bundled: bool) -> Result<String> {
    let name = if bundled {
        local.to_string()
    } else {
        format!("lua__{id}__{local}")
    };
    if name.len() > 64 {
        bail!("plugin tool name exceeds 64 bytes: {name}");
    }
    Ok(name)
}

/// 【插件】【发现】读取受信任目录和显式配置，不执行工作区中的任意插件。
/// @param config 主配置，仅用于旧功能开关默认值；paths 为 Sai 路径
/// @returns 稳定排序的包和独立诊断；配置损坏时不采用可能扩大权限的缺省值
pub(crate) fn discover(config: &AppConfig, paths: &SaiPaths) -> Discovery {
    let mut found = Discovery::default();
    let settings = match load_config(paths) {
        Ok(settings) => settings,
        Err(error) => {
            found.diagnostics.push(diagnostic("plugins.jsonc", error));
            return found;
        }
    };
    let mut ids = BTreeSet::new();
    match bundled::packages() {
        Ok(packages) => {
            for package in packages {
                let id = &package.manifest.id;
                let setting = settings
                    .plugins
                    .get(id)
                    .cloned()
                    .unwrap_or_else(|| PluginSetting {
                        enabled: bundled::default_enabled(config, id),
                        ..Default::default()
                    });
                ids.insert(id.clone());
                found.plugins.push(PluginDescriptor {
                    package,
                    source: PluginSource::Bundled,
                    setting,
                });
            }
        }
        Err(error) => found.diagnostics.push(diagnostic("bundled", error)),
    }
    let root = paths.config_dir.join("plugins");
    let entries = match std::fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return found,
        Err(error) => {
            found
                .diagnostics
                .push(diagnostic(root.display().to_string(), error.into()));
            return found;
        }
    };
    // 【插件】【目录发现】限制扫描数量，临时安装目录不参与加载
    for entry in entries.take(257) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                found
                    .diagnostics
                    .push(diagnostic("plugins directory", error.into()));
                continue;
            }
        };
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if found.plugins.len() >= 256 {
            found.diagnostics.push(diagnostic(
                "plugins directory",
                anyhow::anyhow!("plugin count exceeds 256"),
            ));
            break;
        }
        let path = entry.path();
        let result = (|| {
            let package = PluginPackage::from_directory(&path)?;
            let id = &package.manifest.id;
            if path.file_name().and_then(|name| name.to_str()) != Some(id) {
                bail!("installed directory must match manifest id: {id}");
            }
            if !ids.insert(id.clone()) {
                bail!("plugin id is reserved or duplicated: {id}");
            }
            let setting = settings.plugins.get(id).cloned().unwrap_or_default();
            Ok(PluginDescriptor {
                package,
                source: PluginSource::Installed(path.clone()),
                setting,
            })
        })();
        match result {
            Ok(descriptor) => found.plugins.push(descriptor),
            Err(error) => found
                .diagnostics
                .push(diagnostic(path.display().to_string(), error)),
        }
    }
    found
        .plugins
        .sort_by(|left, right| left.package.manifest.id.cmp(&right.package.manifest.id));
    found
        .diagnostics
        .sort_by(|left, right| left.source.cmp(&right.source));
    found
}

/// 【插件】【选择】从发现结果中选取明确的插件，并保留相关诊断。
/// @param config 主配置；paths 为 Sai 路径；id 为插件 ID
/// @returns 已安装或内置的插件描述
pub(super) fn find(config: &AppConfig, paths: &SaiPaths, id: &str) -> Result<PluginDescriptor> {
    let found = discover(config, paths);
    found
        .plugins
        .into_iter()
        .find(|plugin| plugin.package.manifest.id == id)
        .with_context(|| {
            format!(
                "plugin not found: {id}{}",
                found
                    .diagnostics
                    .iter()
                    .map(|item| format!("\n{}: {}", item.source, item.error))
                    .collect::<String>()
            )
        })
}

/// 【插件】【错误记录】把错误链转换为可显示的单包诊断。
/// @param source 包标识或来源路径；error 为完整错误
/// @returns 不影响其他插件的诊断条目
pub(super) fn diagnostic(source: impl Into<String>, error: anyhow::Error) -> PluginDiagnostic {
    PluginDiagnostic {
        source: source.into(),
        error: format!("{error:#}"),
    }
}
