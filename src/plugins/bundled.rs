use crate::config::AppConfig;
use anyhow::{Context, Result};
use rust_embed::RustEmbed;
use sai_plugin_runtime::{PluginManifest, PluginPackage};
use std::collections::BTreeMap;

#[derive(RustEmbed)]
#[folder = "plugins/"]
struct BundledPlugins;

/// 【插件】【内置发现】从同一资源目录发现所有随程序发布的 Lua 包。
/// @returns 按插件 ID 排序的完整源码快照
pub(super) fn packages() -> Result<Vec<PluginPackage>> {
    let mut packages = Vec::new();
    for file in BundledPlugins::iter().filter(|path| path.ends_with("/sai-plugin.json")) {
        let manifest = PluginManifest::parse(&read(&file)?)?;
        let prefix = file
            .strip_suffix("sai-plugin.json")
            .context("invalid bundled manifest path")?;
        let mut sources = BTreeMap::new();
        for source in
            BundledPlugins::iter().filter(|path| path.starts_with(prefix) && path.ends_with(".lua"))
        {
            sources.insert(source[prefix.len()..].to_string(), read(&source)?);
        }
        packages.push(PluginPackage::new(manifest, sources)?);
    }
    packages.sort_by(|left, right| left.manifest.id.cmp(&right.manifest.id));
    Ok(packages)
}

/// 【插件】【兼容默认值】现有功能开关仅决定迁移插件的缺省启用状态。
/// @param config 主配置；id 为内置插件 ID
/// @returns 未写入 plugins.jsonc 时采用的启用状态
pub(super) fn default_enabled(config: &AppConfig, id: &str) -> bool {
    match id {
        "archlinux" => config.plugins.archlinux.enabled,
        "linux-game-signals" => config.plugins.linux_game_compatibility.enabled,
        "online-man" => config.plugins.man.enabled,
        "web-search" => config.plugins.web.enabled,
        _ => true,
    }
}

/// 【插件】【资源读取】读取随二进制发布的 UTF-8 文件。
/// @param path 资源目录中的相对路径
/// @returns 文件文本
fn read(path: &str) -> Result<String> {
    let file = BundledPlugins::get(path)
        .with_context(|| format!("bundled plugin file not found: {path}"))?;
    Ok(String::from_utf8(file.data.into_owned()).context("bundled plugin is not UTF-8")?)
}
