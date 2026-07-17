use super::model::ContextSourceSnapshot;
use anyhow::Result;
use sha2::{Digest, Sha256};

/// 序列化 Context Source 快照。
///
/// 参数:
/// - `sources`: 稳定 source 快照
///
/// 返回:
/// - JSON 快照文本
pub(crate) fn snapshot_json(sources: &[ContextSourceSnapshot]) -> Result<String> {
    Ok(serde_json::to_string(sources)?)
}

/// 校验 Context Source 快照 JSON。
///
/// 参数:
/// - `value`: 快照 JSON 文本
///
/// 返回:
/// - 校验是否成功
pub(crate) fn validate_snapshot_json(value: &str) -> Result<Vec<ContextSourceSnapshot>> {
    Ok(serde_json::from_str(value)?)
}

/// 计算 baseline hash。
///
/// 参数:
/// - `baseline`: baseline 文本
///
/// 返回:
/// - hash 文本
pub(crate) fn baseline_hash(baseline: &str) -> String {
    hash_text(baseline)
}

/// 计算文本 hash。
///
/// 参数:
/// - `text`: 原始文本
///
/// 返回:
/// - hash 文本
pub(crate) fn hash_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}
