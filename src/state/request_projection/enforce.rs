use super::model::{ProjectedRequest, ProjectionWarning};
use super::validator::first_blocking_tool_pairing_warning;
use crate::state::{FailureKind, RecoveryStatus, StateStore};
use anyhow::{bail, Result};

impl StateStore {
    /// 严格校验 provider 请求投影。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮次标识
    /// - `projection`: 待发送 provider 的请求投影
    ///
    /// 返回:
    /// - 校验是否通过
    pub(crate) fn enforce_provider_projection(
        &self,
        turn_id: Option<&str>,
        projection: &ProjectedRequest,
    ) -> Result<()> {
        let Some(warning) = first_blocking_tool_pairing_warning(projection) else {
            return Ok(());
        };
        let kind = classify_tool_pairing_warning(&warning);
        self.record_recovery_failure(
            turn_id,
            kind,
            RecoveryStatus::Terminal,
            &format!("provider request projection blocked: {}", warning.message),
            0,
            projection.estimate.message_chars,
            projection.estimate.context_limit_chars,
        )?;
        bail!("provider request projection blocked: {}", warning.message)
    }
}

/// 将工具配对 warning 分类为恢复记录类型。
///
/// 参数:
/// - `warning`: 投影校验 warning
///
/// 返回:
/// - 恢复记录类型
fn classify_tool_pairing_warning(warning: &ProjectionWarning) -> FailureKind {
    let message = warning.message.as_str();
    if message.contains("tool call without result") {
        FailureKind::ToolHistoryMissingResult
    } else if message.contains("duplicate tool result")
        || message.contains("duplicate tool call id")
    {
        FailureKind::ToolHistoryDuplicateResult
    } else if message.contains("orphan tool result")
        || message.contains("tool result without call id")
    {
        FailureKind::ToolHistoryOrphanResult
    } else {
        FailureKind::ProjectionInvalid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_tool_pairing_warnings() {
        assert_eq!(
            classify_tool_pairing_warning(&ProjectionWarning {
                message: "provider projection has tool call without result: call_1".to_string(),
            }),
            FailureKind::ToolHistoryMissingResult
        );
        assert_eq!(
            classify_tool_pairing_warning(&ProjectionWarning {
                message: "provider projection has duplicate tool result: call_1".to_string(),
            }),
            FailureKind::ToolHistoryDuplicateResult
        );
        assert_eq!(
            classify_tool_pairing_warning(&ProjectionWarning {
                message: "provider projection has orphan tool result: call_1".to_string(),
            }),
            FailureKind::ToolHistoryOrphanResult
        );
    }
}
