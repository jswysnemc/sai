use super::{discover, host::SaiPluginHost};
use crate::{config::AppConfig, paths::SaiPaths};
use anyhow::{bail, Context, Result};
use sai_plugin_runtime::PluginRuntime;
use serde_json::json;
use std::sync::Arc;

/// 【权限审核】【插件加载】仅加载明确选择、启用且授权的审核插件
/// 参数: config 为本次审核配置，paths 为配置目录；返回独立审核运行时
pub(crate) fn load(config: &AppConfig, paths: &SaiPaths) -> Result<PluginRuntime> {
    let id = config.permission.auto_audit_plugin_id.trim();
    let descriptor = discover(config, paths)
        .plugins
        .into_iter()
        .find(|item| item.package.manifest.id == id)
        .context("configured permission audit plugin is not installed")?;
    if !descriptor.setting.enabled
        || !descriptor.capabilities().permission_audit
        || !descriptor.grants().permission_audit
    {
        bail!("configured permission audit plugin is disabled or not granted");
    }
    let mut settings = descriptor.settings().clone();
    let settings_object = settings
        .as_object_mut()
        .context("permission audit plugin settings must be an object")?;
    let provider_id = config.permission.auto_audit_provider_id.trim();
    let model = config.permission.auto_audit_model.trim();
    if provider_id.is_empty() != model.is_empty() {
        bail!("permission audit provider and model must be configured together");
    }
    if !provider_id.is_empty() {
        let provider = config
            .providers
            .iter()
            .find(|provider| provider.id == provider_id)
            .context("permission audit provider does not exist")?;
        // 1. 【权限审核】【供应商复用】只向已授权审核插件交付所选供应商，不复制持久凭据
        settings_object.insert(
            "provider".into(),
            json!({
                "id": provider.id,
                "base_url": provider.base_url,
                "model": model,
                "api_key": provider.resolved_api_key(paths)?,
            }),
        );
    }
    let runtime = PluginRuntime::load(
        descriptor.runtime_package(),
        settings,
        descriptor.grants(),
        Arc::new(SaiPluginHost),
    )?;
    if !runtime.has_permission_audit() {
        bail!("configured plugin has no permission audit callback");
    }
    Ok(runtime)
}
