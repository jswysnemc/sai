//! 用户、项目和已安装插件的 hooks 定义独立合并，不把运行期条目写回主配置。
use crate::{
    config::{AppConfig, HookItem, HooksConfig},
    paths::SaiPaths,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HookSource {
    pub path: Option<PathBuf>,
    pub root: PathBuf,
    pub digest: String,
    pub files: Vec<String>,
    pub plugin: bool,
    pub managed: bool,
    pub required: bool,
    pub output_dir: PathBuf,
    pub history_dir: PathBuf,
    pub environment: BTreeMap<String, String>,
    pub user_enabled: bool,
    pub item_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SourcePreview {
    pub path: String,
    pub digest: String,
    pub trusted: bool,
    pub managed: bool,
    pub required: bool,
    pub items: Vec<HookItem>,
    pub error: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Policy {
    managed_only: bool,
    required: bool,
    items: Vec<HookItem>,
}

/// 【Hooks】【内容摘要】参数为定义、根目录、声明文件和插件开关，返回包含受监控文件的摘要
pub(super) fn fingerprint(
    file: &Path,
    root: &Path,
    files: &[String],
    plugin: bool,
) -> Result<String> {
    let bytes = super::source_io::read(file, 1_048_576)?;
    fingerprint_snapshot(file, root, files, plugin, &bytes)
}

/// 【Hooks】【定义快照】参数为路径、目录、监控列表、插件开关和已解析字节，返回同一快照的内容摘要
fn fingerprint_snapshot(
    file: &Path,
    root: &Path,
    files: &[String],
    plugin: bool,
    bytes: &[u8],
) -> Result<String> {
    anyhow::ensure!(
        bytes.len() <= 1_048_576 && !std::fs::symlink_metadata(file)?.file_type().is_symlink(),
        "invalid hook definition snapshot"
    );
    let mut digest = blake3::Hasher::new();
    let mut budget = 16_777_216usize - bytes.len();
    digest.update(&(bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    if plugin {
        hash_tree(root, file, &mut digest, &mut budget, &mut 0, 0)?;
    } else {
        for name in files {
            let target = root.join(name).canonicalize()?;
            anyhow::ensure!(
                target.starts_with(root.canonicalize()?),
                "hook monitored file escapes its source root"
            );
            digest.update(name.as_bytes());
            if target != file {
                hash_file(&target, &mut digest, &mut budget)?;
            }
        }
    }
    Ok(digest.finalize().to_hex().to_string())
}

/// 【Hooks】【文件摘要】参数为文件、摘要器和剩余预算，返回有界读取结果，拒绝符号链接
fn hash_file(file: &Path, digest: &mut blake3::Hasher, budget: &mut usize) -> Result<()> {
    let metadata = std::fs::symlink_metadata(file)?;
    anyhow::ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "hook source must be a regular file"
    );
    anyhow::ensure!(
        metadata.len() <= *budget as u64,
        "hook source exceeds 16 MiB"
    );
    let bytes = super::source_io::read(file, *budget)?;
    anyhow::ensure!(bytes.len() <= *budget, "hook source exceeds 16 MiB");
    *budget -= bytes.len();
    digest.update(&(bytes.len() as u64).to_le_bytes());
    digest.update(&bytes);
    Ok(())
}

/// 【Hooks】【插件摘要】参数为目录、摘要器、预算、文件计数和深度，返回确定性完整内容校验
fn hash_tree(
    root: &Path,
    definition: &Path,
    digest: &mut blake3::Hasher,
    budget: &mut usize,
    count: &mut usize,
    depth: usize,
) -> Result<()> {
    anyhow::ensure!(depth < 24, "plugin hook source is too deep");
    let mut entries = std::fs::read_dir(root)?
        .take(4097)
        .collect::<std::io::Result<Vec<_>>>()?;
    anyhow::ensure!(
        entries.len() <= 4096,
        "plugin hook source contains too many files"
    );
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        *count += 1;
        anyhow::ensure!(*count <= 4096, "plugin hook source contains too many files");
        digest.update(entry.file_name().to_string_lossy().as_bytes());
        let kind = entry.file_type()?;
        anyhow::ensure!(
            !kind.is_symlink(),
            "plugin hook source contains a symbolic link"
        );
        if kind.is_dir() {
            hash_tree(&entry.path(), definition, digest, budget, count, depth + 1)?;
        } else if entry.path() != definition {
            hash_file(&entry.path(), digest, budget)?;
        }
    }
    Ok(())
}

/// 【Hooks】【定义读取】参数为不可变 JSONC 字节，返回 Sai 列表或 Codex 事件分组转换后的条目
fn read_items(bytes: &[u8]) -> Result<Vec<HookItem>> {
    anyhow::ensure!(bytes.len() <= 1_048_576, "hook definitions exceed 1 MiB");
    let value: serde_json::Value =
        serde_json::from_reader(json_comments::StripComments::new(&bytes[..]))?;
    if let Some(items) = value.get("items") {
        return Ok(serde_json::from_value(items.clone())?);
    }
    let groups = value
        .get("hooks")
        .and_then(serde_json::Value::as_object)
        .context("expected items or hooks event map")?;
    let mut items = Vec::new();
    for (event, groups) in groups {
        for group in groups.as_array().context("hook groups must be arrays")? {
            for handler in group["hooks"]
                .as_array()
                .context("hook handlers must be arrays")?
            {
                // 1. 【Hooks】【格式兼容】保留 Codex 命名字段，所有动作仍通过 Sai 协议校验
                let mut item = handler
                    .as_object()
                    .context("hook handler must be an object")?
                    .clone();
                if let Some(kind) = item.remove("type") {
                    item.insert("kind".into(), kind);
                }
                if let Some(script) = item.remove("command") {
                    item.insert("script".into(), script);
                }
                if let Some(script) = item.remove("commandWindows") {
                    item.insert("command_windows".into(), script);
                }
                if let Some(message) = item.remove("statusMessage") {
                    item.insert("status_message".into(), message);
                }
                if let Some(seconds) = item.remove("timeout").and_then(|value| value.as_u64()) {
                    item.insert("timeout_ms".into(), (seconds.saturating_mul(1000)).into());
                }
                item.insert("event".into(), event.clone().into());
                item.entry("name")
                    .or_insert_with(|| format!("{event}-{}", items.len() + 1).into());
                if let Some(matcher) = group.get("matcher") {
                    item.insert("matcher".into(), matcher.clone());
                }
                items.push(serde_json::from_value(item.into())?);
            }
        }
    }
    Ok(items)
}

/// 【Hooks】【来源预览】参数为配置、路径和工作区，返回经过校验但不执行的来源清单及托管策略
fn discover(
    config: &AppConfig,
    paths: &SaiPaths,
    cwd: &Path,
) -> Result<(Vec<SourcePreview>, bool)> {
    let policy_file = PathBuf::from("/etc/sai/hooks.json");
    let policy_bytes = if policy_file.exists() {
        super::trust::managed_file(&policy_file)?;
        let bytes = super::source_io::read(&policy_file, 1_048_576)?;
        anyhow::ensure!(bytes.len() <= 1_048_576, "managed hooks exceed 1 MiB");
        Some(bytes)
    } else {
        None
    };
    let policy = policy_bytes
        .as_deref()
        .map(serde_json::from_slice::<Policy>)
        .transpose()?
        .unwrap_or_default();
    let trusted = super::trust::load(paths)?;
    let mut locations = vec![
        (paths.config_dir.join("hooks.json"), false, false),
        (cwd.join(".sai/hooks.json"), false, false),
    ];
    for plugin in crate::plugins::discover(config, paths).plugins {
        if plugin.setting.enabled {
            let crate::plugins::PluginSource::Installed(root) = plugin.source;
            locations.push((root.join("hooks.json"), true, false));
        }
    }
    if policy_file.exists() {
        locations.insert(0, (policy_file, false, true));
    }
    let mut found = Vec::new();
    for (file, plugin, managed) in locations {
        if !file.exists() {
            continue;
        }
        let result = (|| -> Result<SourcePreview> {
            anyhow::ensure!(
                !std::fs::symlink_metadata(&file)?.file_type().is_symlink(),
                "hook source must not be a symbolic link"
            );
            let file = file.canonicalize()?;
            let root = file.parent().context("hook source directory")?;
            let snapshot = if managed {
                policy_bytes.clone().context("managed hook snapshot")?
            } else {
                super::source_io::read(&file, 1_048_576)?
            };
            let items = if managed {
                policy.items.clone()
            } else {
                read_items(&snapshot)?
            };
            super::validate(&HooksConfig {
                enabled: true,
                items: items.clone(),
            })?;
            let files = items
                .iter()
                .flat_map(|item| item.options.files.iter().cloned())
                .collect::<Vec<_>>();
            let digest = fingerprint_snapshot(&file, root, &files, plugin, &snapshot)?;
            let path = file.display().to_string();
            let is_trusted = managed || trusted.get(&path) == Some(&digest);
            Ok(SourcePreview {
                path,
                digest,
                trusted: is_trusted,
                managed,
                required: managed && policy.required,
                items,
                error: None,
            })
        })();
        match result {
            Ok(source) => found.push(source),
            Err(error) if managed && policy.required => return Err(error),
            Err(error) => found.push(SourcePreview {
                path: file.display().to_string(),
                digest: String::new(),
                trusted: false,
                managed,
                required: false,
                items: Vec::new(),
                error: Some(format!("{error:#}")),
            }),
        }
    }
    Ok((found, policy.managed_only))
}

/// 【Hooks】【配置预览】参数为主配置、应用目录和工作区，返回来源、定义和待授权内容摘要
pub(crate) fn preview(
    config: &AppConfig,
    paths: &SaiPaths,
    cwd: &Path,
) -> Result<Vec<SourcePreview>> {
    Ok(discover(config, paths, cwd)?.0)
}

/// 【Hooks】【运行期合并】参数为主配置、目录与工作区，返回不重复且不持久化外部条目的有效配置
pub(crate) fn resolve(config: &AppConfig, paths: &SaiPaths, cwd: &Path) -> Result<HooksConfig> {
    let user_enabled = config
        .hooks
        .items
        .iter()
        .find_map(|item| {
            item.options
                .runtime
                .as_ref()
                .map(|source| source.user_enabled)
        })
        .unwrap_or(config.hooks.enabled);
    let (sources, managed_only) = discover(config, paths, cwd)?;
    let environment = std::env::vars_os()
        .filter_map(|(key, value)| Some((key.into_string().ok()?, value.into_string().ok()?)))
        .filter(|(key, _)| !key.starts_with("SAI_INTERNAL_") && !key.starts_with("SAI_SANDBOX_"))
        .collect();
    let base = HookSource {
        path: None,
        root: cwd.into(),
        digest: String::new(),
        files: Vec::new(),
        plugin: false,
        managed: false,
        required: false,
        output_dir: paths.data_dir.join("hook-output"),
        history_dir: paths.data_dir.join("hook-history"),
        environment,
        user_enabled,
        item_enabled: true,
    };
    let mut effective = config.hooks.clone();
    effective.items.retain(|item| {
        item.options
            .runtime
            .as_ref()
            .is_none_or(|source| source.path.is_none())
    });
    if managed_only {
        effective.items.clear();
    }
    for item in &mut effective.items {
        let enabled = item
            .options
            .runtime
            .as_ref()
            .map_or(item.enabled, |source| source.item_enabled);
        item.enabled = enabled && user_enabled;
        item.options.runtime = Some(HookSource {
            item_enabled: enabled,
            ..base.clone()
        });
        if item.options.shell.is_none() && !config.terminal.shell.trim().is_empty() {
            item.options.shell = Some(config.terminal.shell.clone());
        }
    }
    for source in sources {
        if !source.trusted || managed_only && !source.managed {
            eprintln!("【Hooks】【来源审查】尚未授权或策略禁用: {}", source.path);
            continue;
        }
        let file = PathBuf::from(&source.path);
        let plugin = file.starts_with(paths.config_dir.join("plugins"));
        let files = source
            .items
            .iter()
            .flat_map(|item| item.options.files.iter().cloned())
            .collect();
        let runtime = HookSource {
            path: Some(file.clone()),
            root: file.parent().context("hook source root")?.into(),
            digest: source.digest,
            files,
            plugin,
            managed: source.managed,
            required: source.required,
            ..base.clone()
        };
        for mut item in source.items {
            if !source.managed && !user_enabled {
                item.enabled = false;
            }
            if source.managed {
                item.enabled = true;
                effective.enabled = true;
            }
            item.options.runtime = Some(runtime.clone());
            if item.options.shell.is_none() && !config.terminal.shell.trim().is_empty() {
                item.options.shell = Some(config.terminal.shell.clone());
            }
            effective.items.push(item);
        }
    }
    Ok(effective)
}

/// 【Hooks】【执行前复核】参数为单项定义，返回来源是否仍与审查时一致，插件与声明文件都会重新读取
pub(super) fn revalidate(item: &HookItem) -> Result<()> {
    if let Some(source) = &item.options.runtime {
        if let Some(file) = &source.path {
            anyhow::ensure!(
                fingerprint(file, &source.root, &source.files, source.plugin)? == source.digest,
                "hook source changed after review: {}",
                file.display()
            );
            if source.managed {
                super::trust::managed_file(file)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn definition_digest_uses_the_same_bytes_as_preview() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("hooks.json");
        let safe = br#"{"items":[]}"#;
        std::fs::write(&file, safe).unwrap();
        let expected = fingerprint(&file, root.path(), &[], false).unwrap();
        std::fs::write(
            &file,
            br#"{"items":[{"name":"changed","event":"Stop","script":"exit 2"}]}"#,
        )
        .unwrap();
        assert_eq!(
            expected,
            fingerprint_snapshot(&file, root.path(), &[], false, safe).unwrap()
        );
        assert_ne!(
            expected,
            fingerprint(&file, root.path(), &[], false).unwrap()
        );
    }
}
