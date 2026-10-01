use super::config_service::{ensure_secret_sentinels_resolved, merge_secret_sentinels_json};
use crate::config::{AppConfig, ProviderConfig};
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;

/// 【供应商配置】【草稿身份】保留已保存对象的标识，允许改名后继续测试连接。
#[derive(Deserialize)]
pub(crate) struct ProviderDraft {
    #[serde(flatten)]
    provider: ProviderConfig,
    #[serde(default)]
    original_id: Option<String>,
}

impl ProviderDraft {
    /// 【供应商配置】【草稿凭据】按原始标识恢复哨兵，保留当前草稿中的新 ID 和接入参数。
    /// 参数: paths 为配置路径；返回: 可用于探测的供应商配置
    pub(crate) fn restore(self, paths: &SaiPaths) -> Result<ProviderConfig> {
        let current = AppConfig::load_or_default(paths)?;
        let source = self.original_id.as_deref().unwrap_or(&self.provider.id);
        let mut submitted = serde_json::to_value(&self.provider)?;
        if let Some(provider) = current
            .providers
            .iter()
            .find(|provider| provider.id == source)
        {
            merge_secret_sentinels_json(&mut submitted, &serde_json::to_value(provider)?);
        }
        ensure_secret_sentinels_resolved(&submitted)?;
        Ok(serde_json::from_value(submitted)?)
    }
}

/// 【供应商配置】【草稿身份】为响应添加原始 ID，仅在 Web 草稿中保留。
/// 参数: config 为脱敏响应；返回: 无
pub(super) fn annotate_sources(config: &mut Value) {
    let Some(providers) = config.get_mut("providers").and_then(Value::as_array_mut) else {
        return;
    };
    for provider in providers {
        if let Some(object) = provider.as_object_mut() {
            if let Some(id) = object.get("id").cloned() {
                object.insert("original_id".into(), id);
            }
        }
    }
}

/// 【供应商配置】【改名保存】按明确的原始 ID 恢复凭据，移除响应专用字段。
/// 参数: submitted 为待保存配置，current 为磁盘快照；返回: 来源是否合法
pub(super) fn restore_sources(submitted: &mut Value, current: &Value) -> Result<()> {
    let Some(providers) = submitted.get_mut("providers").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    let mut sources = HashSet::new();
    for provider in providers {
        let Some(source) = provider
            .as_object_mut()
            .and_then(|object| object.remove("original_id"))
        else {
            continue;
        };
        let source = source
            .as_str()
            .context("provider original_id must be a string")?;
        if !sources.insert(source.to_string()) {
            bail!("duplicate provider original_id: {source}");
        }
        let original = current["providers"]
            .as_array()
            .and_then(|providers| {
                providers
                    .iter()
                    .find(|item| item["id"].as_str() == Some(source))
            })
            .with_context(|| format!("original provider no longer exists: {source}"))?;
        // 1. 按对象来源恢复，禁止用数组位置推断或从其他供应商借用密钥
        merge_secret_sentinels_json(provider, original);
    }
    Ok(())
}
