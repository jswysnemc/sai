use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContextEpoch {
    pub session_id: String,
    pub baseline: String,
    pub baseline_hash: String,
    pub snapshot_json: String,
    pub source_count: usize,
    pub last_change_reason: ContextChangeReason,
    pub blocked_source: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextEpochSummary {
    pub baseline_hash: String,
    pub source_count: usize,
    pub last_change_reason: String,
    pub blocked_source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextEpochProjection {
    pub baseline: String,
    pub baseline_hash: String,
    pub source_count: usize,
    pub last_change_reason: String,
    pub blocked_source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ContextChangeReason {
    Initialized,
    StableSourceChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ContextSourceSnapshot {
    pub key: String,
    pub text_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSourceInput {
    pub(crate) key: String,
    pub(crate) state: ContextSourceInputState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ContextSourceInputState {
    Available(String),
    #[allow(dead_code)]
    Blocked(String),
}

impl ContextSourceInput {
    /// 构造可用 Context Source 输入。
    ///
    /// 参数:
    /// - `key`: source 稳定键
    /// - `text`: source 文本内容
    ///
    /// 返回:
    /// - Context Source 输入
    pub fn available(key: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            state: ContextSourceInputState::Available(text.into()),
        }
    }

    /// 构造不可用 Context Source 输入。
    ///
    /// 参数:
    /// - `key`: source 稳定键
    /// - `reason`: source 不可用原因
    ///
    /// 返回:
    /// - Context Source 输入
    #[allow(dead_code)]
    pub fn blocked(key: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            state: ContextSourceInputState::Blocked(reason.into()),
        }
    }
}

/// 转换变更原因为数据库文本。
///
/// 参数:
/// - `reason`: Context Epoch 变更原因
///
/// 返回:
/// - 数据库文本
pub(crate) fn reason_to_str(reason: &ContextChangeReason) -> &'static str {
    match reason {
        ContextChangeReason::Initialized => "initialized",
        ContextChangeReason::StableSourceChanged => "stable_source_changed",
    }
}

/// 从数据库文本恢复变更原因。
///
/// 参数:
/// - `value`: 数据库文本
///
/// 返回:
/// - Context Epoch 变更原因
pub(crate) fn reason_from_str(value: &str) -> ContextChangeReason {
    match value {
        "stable_source_changed" => ContextChangeReason::StableSourceChanged,
        _ => ContextChangeReason::Initialized,
    }
}
