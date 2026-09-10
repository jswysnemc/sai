use super::record::LegacyRecord;
use crate::plugins::{
    discovery::{self, PluginSource},
    private::PrivatePluginHost,
};
use crate::{config::AppConfig, paths::SaiPaths};
use anyhow::{ensure, Context, Result};
use sai_plugin_runtime::PluginRuntime;
use std::sync::Arc;

/// 【旧闹钟兼容】【到期加载】只允许当前启用的可信内置包承接旧任务，不加载工作区源码。
/// @param paths 可信路径；record 为已完成入口身份核验的旧任务
/// @returns 受当前调度和通知授权约束的 Lua 实例
pub(super) fn load(paths: &SaiPaths, record: &LegacyRecord) -> Result<PluginRuntime> {
    let config = AppConfig::load_or_default(paths)?;
    let descriptor = discovery::find(&config, paths, "alarm")?;
    ensure!(
        matches!(descriptor.source, PluginSource::Bundled),
        "legacy alarms require the bundled alarm plugin"
    );
    ensure!(descriptor.setting.enabled, "alarm plugin is disabled");
    let mut package = descriptor.runtime_package();
    let mut grants = descriptor.grants();
    // 1. 【旧闹钟兼容】【精确旧授权】只在用户未配置新读取范围时保留已批准旧音频的单文件许可
    if descriptor.setting.grants.is_none()
        && descriptor.setting.settings.get("audio_paths").is_none()
    {
        if let Some(path) = &record.audio_file {
            let canonical = dunce::canonicalize(path).context("resolve legacy alarm audio")?;
            ensure!(
                dunce::simplified(path) == dunce::simplified(&canonical),
                "legacy alarm audio path changed to a symbolic link"
            );
            ensure!(
                std::fs::symlink_metadata(path)?.is_file(),
                "legacy alarm audio requires a regular file"
            );
            let path = dunce::simplified(&canonical)
                .to_str()
                .context("legacy alarm audio path is not UTF-8")?
                .to_string();
            package
                .manifest
                .capabilities
                .system
                .read_paths
                .insert(path.clone());
            grants.system.read_paths.insert(path);
        }
    }
    // 2. 【旧闹钟兼容】【当前撤权】显式配置永远优先，局部授权不保存到配置或其他插件实例
    let effective = package.manifest.capabilities.intersection(&grants);
    ensure!(
        effective.system.schedule,
        "alarm scheduling grant was revoked"
    );
    ensure!(
        effective.system.notify,
        "alarm notification grant was revoked"
    );
    let host = Arc::new(PrivatePluginHost::with_revision(
        paths,
        "alarm",
        descriptor.revision()?,
    ));
    PluginRuntime::load(package, descriptor.settings().clone(), grants, host)
}
