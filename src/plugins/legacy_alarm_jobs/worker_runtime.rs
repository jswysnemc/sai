use super::record::LegacyRecord;
use crate::plugins::{discovery, private::PrivatePluginHost};
use crate::{config::AppConfig, paths::SaiPaths};
use anyhow::{ensure, Result};
use sai_plugin_runtime::PluginRuntime;
use std::sync::Arc;

/// 【旧闹钟兼容】【到期加载】存量工作进程使用当前已安装插件及显式授权。
/// @param paths 可信路径；record 为已完成入口身份核验的旧任务
/// @returns 受当前调度和通知授权约束的 Lua 实例
pub(super) fn load(paths: &SaiPaths, _record: &LegacyRecord) -> Result<PluginRuntime> {
    let config = AppConfig::load_or_default(paths)?;
    let descriptor = discovery::find(&config, paths, "alarm")?;
    ensure!(descriptor.setting.enabled, "alarm plugin is disabled");
    let package = descriptor.runtime_package();
    let grants = descriptor.grants();
    // 【旧闹钟兼容】【当前撤权】旧音频文件也须符合当前清单与用户授权
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
