use super::bundled;
use super::config::{load_config, mutation_lock, save_config, PluginSetting};
use super::discovery::{find, public_tool_name, PluginSource};
use super::grants::GrantUpdate;
use super::host::SaiPluginHost;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use sai_plugin_runtime::{
    Capabilities, EventKind, PluginCommand, PluginManifest, PluginPackage, PluginRuntime,
    PluginTool,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// 【插件】【检查结果】来自真实入口加载的工具、命令和事件清单。
#[derive(Serialize)]
pub(crate) struct PluginInspection {
    pub manifest: PluginManifest,
    pub tools: Vec<PluginTool>,
    pub commands: Vec<PluginCommand>,
    pub events: Vec<EventKind>,
}

/// 【插件】【包验证】执行受限初始化并检查实际导出的契约。
/// @param directory 插件源码目录
/// @returns 可序列化的检查结果，不安装、不启用、不提供 HTTP 授权
pub(crate) fn validate_package(directory: &Path) -> Result<PluginInspection> {
    inspect_package(PluginPackage::from_directory(directory)?)
}

/// 【插件】【初始化检查】与运行时使用同一清单、模块、资源和注册校验。
/// @param package 固定源码快照
/// @returns 实际工具、命令和事件定义
fn inspect_package(package: PluginPackage) -> Result<PluginInspection> {
    let runtime = PluginRuntime::load(
        package,
        json!({}),
        Capabilities::default(),
        Arc::new(SaiPluginHost),
    )?;
    for tool in runtime.tools() {
        public_tool_name(&runtime.manifest().id, &tool.name, false)?;
    }
    Ok(PluginInspection {
        manifest: runtime.manifest().clone(),
        tools: runtime.tools().to_vec(),
        commands: runtime.commands().to_vec(),
        events: runtime.events().to_vec(),
    })
}

/// 【插件】【安装事务】只复制已验证的 Lua 源码和规范清单，新安装的包保持禁用。
/// @param directory 源码目录；paths 为 Sai 路径；replace 表示显式允许替换既有安装
/// @returns 安装后的路径；配置保存失败时恢复原目录
pub(crate) fn install(directory: &Path, paths: &SaiPaths, replace: bool) -> Result<PathBuf> {
    let _lock = mutation_lock(paths)?;
    let package = PluginPackage::from_directory(directory)?;
    inspect_package(package.clone())?;
    let id = &package.manifest.id;
    if bundled::packages()?
        .iter()
        .any(|bundled| bundled.manifest.id == *id)
    {
        bail!("cannot replace bundled plugin: {id}");
    }
    let mut config = load_config(paths)?;
    let root = paths.config_dir.join("plugins");
    std::fs::create_dir_all(&root)?;
    let destination = root.join(id);
    let exists = match std::fs::symlink_metadata(&destination) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                bail!("installed plugin path must be a real directory");
            }
            if !replace {
                bail!("plugin already installed: {id}; use --replace to update it");
            }
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    // 1. 【插件】【安装准备】在同一文件系统暂存完整源码，未完成目录不参与发现
    let staging = tempfile::Builder::new()
        .prefix(".install-")
        .tempdir_in(&root)?;
    write_package(staging.path(), &package)?;
    let backup = tempfile::Builder::new()
        .prefix(".backup-")
        .tempdir_in(&root)?;
    let previous = backup.path().join("previous");
    if exists {
        std::fs::rename(&destination, &previous).context("stage previous plugin version")?;
    }
    let commit = (|| {
        std::fs::rename(staging.path(), &destination).context("install plugin source snapshot")?;
        if !exists {
            config.plugins.insert(id.clone(), PluginSetting::default());
        }
        save_config(paths, &config)
    })();
    if let Err(error) = commit {
        // 2. 【插件】【安装回滚】恢复已有版本，避免配置失败留下半安装状态
        if destination.exists() {
            std::fs::remove_dir_all(&destination).context("roll back new plugin directory")?;
        }
        if exists {
            std::fs::rename(&previous, &destination).context("restore previous plugin version")?;
        }
        return Err(error);
    }
    Ok(destination)
}

/// 【插件】【启停配置】显式配置优先于内置兼容开关，启用本身不自动扩大外部授权。
/// @param config 主配置；paths 为 Sai 路径；id 为插件 ID；enabled 为启用状态；update 为授权修改
/// @returns 保存结果，新会话或显式重新加载后生效
pub(crate) fn set_enabled(
    config: &AppConfig,
    paths: &SaiPaths,
    id: &str,
    enabled: bool,
    update: GrantUpdate,
) -> Result<()> {
    let _lock = mutation_lock(paths)?;
    let descriptor = find(config, paths, id)?;
    let mut plugin_config = load_config(paths)?;
    let mut setting = descriptor.setting.clone();
    setting.enabled = enabled;
    if let Some(grants) = update.resolve(descriptor.grants(), descriptor.capabilities())? {
        setting.grants = Some(grants);
    }
    plugin_config.plugins.insert(id.to_string(), setting);
    save_config(paths, &plugin_config)
}

/// 【插件】【设置保存】仅替换当前插件设置，保持其他包的配置与授权。
/// @param config 主配置；paths 为 Sai 路径；id 为插件 ID；settings 为 JSON 对象
/// @returns 保存结果，非法设置不改写原配置
pub(crate) fn configure(
    config: &AppConfig,
    paths: &SaiPaths,
    id: &str,
    settings: Value,
) -> Result<()> {
    let _lock = mutation_lock(paths)?;
    if !settings.is_object() {
        bail!("plugin settings must be a JSON object");
    }
    let mut descriptor = find(config, paths, id)?;
    descriptor.setting.settings = settings;
    descriptor.refresh_compatibility(config)?;
    // 【插件】【配置验证】校验派生设置但只保存原始设置，环境凭据不会写回配置
    PluginRuntime::load(
        descriptor.runtime_package(),
        descriptor.settings().clone(),
        descriptor.grants(),
        Arc::new(SaiPluginHost),
    )?;
    let mut plugin_config = load_config(paths)?;
    plugin_config
        .plugins
        .insert(id.to_string(), descriptor.setting);
    save_config(paths, &plugin_config)
}

/// 【插件】【卸载】先禁用再移除已确认归属的安装目录。
/// @param config 主配置；paths 为 Sai 路径；id 为插件 ID
/// @returns 卸载结果；随程序发布的包只能禁用
pub(crate) fn remove(config: &AppConfig, paths: &SaiPaths, id: &str) -> Result<()> {
    let _lock = mutation_lock(paths)?;
    let descriptor = find(config, paths, id)?;
    let PluginSource::Installed(directory) = descriptor.source else {
        bail!("bundled plugins can only be disabled");
    };
    let mut plugin_config = load_config(paths)?;
    let mut setting = descriptor.setting;
    setting.enabled = false;
    setting.grants = Some(Capabilities::default());
    plugin_config.plugins.insert(id.to_string(), setting);
    save_config(paths, &plugin_config)?;
    std::fs::remove_dir_all(directory).context("remove installed plugin")
}

/// 【插件】【项目创建】生成包含工具、命令和事件的可执行 Lua 插件模板。
/// @param directory 新目录；id 为插件标识
/// @returns 模板目录，不覆盖现有文件
pub(crate) fn scaffold(directory: &Path, id: &str) -> Result<PathBuf> {
    let manifest = PluginManifest::parse(&json!({
        "api_version": 1, "id": id, "version": "0.1.0", "name": id,
        "description": "A Lua extension with a tool, command and session events.", "entry": "init.lua"
    }).to_string())?;
    let package = PluginPackage::new(
        manifest,
        BTreeMap::from([("init.lua".into(), include_str!("templates/init.lua").into())]),
    )?;
    inspect_package(package.clone())?;
    if std::fs::symlink_metadata(directory).is_ok() {
        bail!("plugin project directory already exists");
    }
    let parent = directory
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let staging = tempfile::Builder::new()
        .prefix(".sai-plugin-")
        .tempdir_in(parent)?;
    write_package(staging.path(), &package)?;
    std::fs::rename(staging.path(), directory).context("create plugin project")?;
    Ok(directory.to_path_buf())
}

/// 【插件】【源码落盘】把已验证快照写入独立暂存目录。
/// @param root 空的目标目录；package 为规范插件包
/// @returns 写入结果，不复制工作区其他文件
fn write_package(root: &Path, package: &PluginPackage) -> Result<()> {
    std::fs::write(
        root.join("sai-plugin.json"),
        format!("{}\n", serde_json::to_string_pretty(&package.manifest)?),
    )?;
    for (relative, source) in package.sources() {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, source)?;
    }
    Ok(())
}
