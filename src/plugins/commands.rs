use super::{discovery::find, private::PrivatePluginHost};
use crate::{config::AppConfig, paths::SaiPaths};
use anyhow::{ensure, Context, Result};
use sai_plugin_runtime::{InvocationContext, PluginRuntime};
use std::sync::Arc;

/// 【插件命令】【显式请求】应用入口与普通插件命令共用权限边界
pub(crate) struct PluginCommand {
    pub plugin: &'static str,
    pub command: &'static str,
    pub arguments: serde_json::Value,
    pub allow_writes: bool,
}

/// 【插件命令】【安装入口】只调用已安装并启用的包，不修改清单或授权
/// @param config 当前配置；paths 为应用目录；request 为已解析 CLI 请求
/// @returns Lua 命令的完整输出；禁用、授权与预算错误继续传播
pub(crate) async fn run_installed(
    config: &AppConfig,
    paths: &SaiPaths,
    request: PluginCommand,
) -> Result<String> {
    let descriptor = find(config, paths, request.plugin)?;
    ensure!(
        descriptor.setting.enabled,
        "plugin is disabled: {}",
        request.plugin
    );
    // 1. 【插件命令】【固定后台身份】任务使用当前源码、设置与显式授权
    let host = Arc::new(PrivatePluginHost::for_descriptor(paths, &descriptor)?);
    let runtime = PluginRuntime::load(
        descriptor.runtime_package(),
        descriptor.settings().clone(),
        descriptor.grants(),
        host,
    )?;
    // 2. 【插件命令】【单次作用域】独立实例只执行这次命令
    runtime
        .call_command(
            request.command,
            &serde_json::to_string(&request.arguments)?,
            InvocationContext {
                session_id: format!("command/{}", request.plugin),
                workdir: std::env::current_dir()?
                    .to_str()
                    .context("command workdir must be UTF-8")?
                    .into(),
                allow_writes: request.allow_writes,
                progress: Some(Arc::new(|message| eprintln!("{message}"))),
                ..Default::default()
            },
        )
        .await
}
