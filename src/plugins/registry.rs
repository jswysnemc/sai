use super::discovery::{diagnostic, discover, PluginDescriptor, PluginDiagnostic, PluginSource};
use super::private::PrivatePluginHost;
use super::session::{PluginInstance, PluginSession};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::tools::{ToolPermission, ToolRegistry, ToolSpec};
use anyhow::{bail, Result};
use sai_plugin_runtime::host::PluginHost;
use std::sync::Arc;

/// 【插件】【共用注册】通过一个入口接入 CLI、TUI、Web 和子任务使用的工具表。
/// @param registry 目标注册表；config 为旧功能默认配置；paths 为插件目录；readonly 决定是否公开写入工具
/// @returns 加载失败诊断；失败插件不会留下工具、命令或监听器
pub(crate) fn register_plugins(
    registry: &mut ToolRegistry,
    config: &AppConfig,
    paths: &SaiPaths,
    readonly: bool,
) -> Vec<PluginDiagnostic> {
    registry.configure_plugin_model(config, paths);
    let found = discover(config, paths);
    let mut diagnostics = found.diagnostics;
    for descriptor in found
        .plugins
        .into_iter()
        .filter(|plugin| plugin.setting.enabled)
    {
        let id = descriptor.package.manifest.id.clone();
        if let Err(error) = register_descriptor(
            registry,
            descriptor,
            Arc::new(PrivatePluginHost::new(paths, &id)),
            readonly,
        ) {
            diagnostics.push(diagnostic(id, error));
        }
    }
    registry.set_plugin_diagnostics(diagnostics.clone());
    diagnostics
}

/// 【插件】【兼容目录】允许为已禁用的内置工具预先设置 Agent 白名单，不执行禁用的外部代码。
/// @param registry 目录注册表；config 为目录配置；paths 为插件配置路径
/// @returns 无；错误保留在目录诊断中
pub(crate) fn register_bundled_catalog_tools(
    registry: &mut ToolRegistry,
    config: &AppConfig,
    paths: &SaiPaths,
) {
    let found = discover(config, paths);
    for mut descriptor in found
        .plugins
        .into_iter()
        .filter(|plugin| !plugin.setting.enabled && matches!(plugin.source, PluginSource::Bundled))
    {
        descriptor.setting.enabled = true;
        let id = descriptor.package.manifest.id.clone();
        if let Err(error) = register_descriptor(
            registry,
            descriptor,
            Arc::new(PrivatePluginHost::new(paths, &id)),
            false,
        ) {
            let mut diagnostics = registry.plugin_diagnostics().to_vec();
            diagnostics.push(diagnostic(id, error));
            registry.set_plugin_diagnostics(diagnostics);
        }
    }
}

/// 【插件】【注册事务】先检查完整包，再一次性提交工具和实例。
/// @param registry 目标表；descriptor 为固定加载输入；host 为能力实现；readonly 限制公开工具
/// @returns 冲突或加载错误，不覆盖现有工具
pub(super) fn register_descriptor(
    registry: &mut ToolRegistry,
    descriptor: PluginDescriptor,
    host: Arc<dyn PluginHost>,
    readonly: bool,
) -> Result<()> {
    let instance = PluginInstance::load(descriptor, host)?;
    let id = instance.runtime.manifest().id.clone();
    let mut specs = Vec::new();
    for definition in instance.runtime.tools() {
        let name = instance.descriptor.tool_name(&definition.name)?;
        if registry.contains(&name) {
            bail!("plugin {id} tool conflicts with an existing tool: {name}");
        }
        let spec = ToolSpec::plugin(&id, name, definition);
        if !readonly || spec.permission == ToolPermission::ReadOnly {
            specs.push(spec);
        }
    }
    let mut session = PluginSession::default();
    session.insert(instance);
    registry.add_plugin_session(&session, &id)?;
    for spec in specs {
        registry.register(spec);
    }
    Ok(())
}
