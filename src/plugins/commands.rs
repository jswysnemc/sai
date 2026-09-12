use super::{
    discovery::{find, PluginSource},
    private::PrivatePluginHost,
};
use crate::{config::AppConfig, paths::SaiPaths};
use anyhow::{ensure, Context, Result};
use sai_plugin_runtime::{InvocationContext, PluginRuntime};
use std::{path::PathBuf, sync::Arc};

/// 【插件命令】【可信请求】输入路径只能由显式 CLI 适配构造，不来自模型工具参数
pub(crate) struct BundledCommand {
    pub plugin: &'static str,
    pub command: &'static str,
    pub arguments: serde_json::Value,
    pub input: Option<PathBuf>,
    pub allow_writes: bool,
}

/// 【插件命令】【兼容入口】临时授权明确输入，不修改插件配置或后续后台任务授权
/// @param config 当前配置；paths 为应用目录；request 为已解析 CLI 请求
/// @returns Lua 命令的完整输出；禁用、授权与预算错误继续传播
pub(crate) async fn run_bundled(
    config: &AppConfig,
    paths: &SaiPaths,
    request: BundledCommand,
) -> Result<String> {
    let descriptor = find(config, paths, request.plugin)?;
    ensure!(
        matches!(descriptor.source, PluginSource::Bundled),
        "compatibility command requires a bundled plugin"
    );
    ensure!(
        descriptor.setting.enabled,
        "plugin is disabled: {}",
        request.plugin
    );
    // 1. 【插件命令】【固定后台身份】宿主使用正常描述符修订，临时导入来源不进入持久任务
    let host = Arc::new(PrivatePluginHost::for_descriptor(paths, &descriptor)?);
    let mut package = descriptor.runtime_package();
    let mut grants = descriptor.grants().intersection(descriptor.capabilities());
    if let Some(input) = request.input {
        let canonical = dunce::canonicalize(input).context("resolve explicit command input")?;
        let text = canonical
            .to_str()
            .context("command input path must be UTF-8")?
            .to_string();
        package
            .manifest
            .capabilities
            .system
            .read_paths
            .insert(text.clone());
        grants.system.read_paths.insert(text);
    }
    package.manifest.capabilities.validate()?;
    grants.validate()?;
    let runtime = PluginRuntime::load(package, descriptor.settings().clone(), grants, host)?;
    // 2. 【插件命令】【单次作用域】独立实例只执行这次命令，不把临时权限放入 Agent 工具表
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
