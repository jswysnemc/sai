use super::RuntimeOverrides;
use crate::config::NotificationConfig;
use anyhow::{Context, Result};
use sai_plugin_runtime::Capabilities;
use serde_json::{json, Value};

/// 【通知兼容】【运行设置】旧桌面通知与提示音设置仅作为对应内置包的缺省值。
/// @param legacy 旧通知配置；settings 为显式设置；declared 为包声明
/// @returns 按字段覆盖的执行快照，不将旧默认值写入 plugins.jsonc
pub(super) fn resolve(
    legacy: &NotificationConfig,
    settings: &Value,
    declared: &Capabilities,
) -> Result<RuntimeOverrides> {
    let mut merged = json!({"enabled": legacy.enabled, "sound": legacy.sound});
    let object = merged.as_object_mut().expect("constructed settings object");
    for (name, value) in settings
        .as_object()
        .context("reply-notification settings must be an object")?
    {
        object.insert(name.clone(), value.clone());
    }
    Ok(RuntimeOverrides {
        settings: merged,
        capabilities: declared.clone(),
    })
}
