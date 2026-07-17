use super::model::{FailureKind, RecoverySnapshot, RecoveryStatus};

/// 判断恢复快照是否有可显示内容。
///
/// 参数:
/// - `snapshot`: 当前恢复快照
///
/// 返回:
/// - 是否需要在命令摘要中显示
pub(crate) fn has_visible_recovery(snapshot: &RecoverySnapshot) -> bool {
    snapshot.latest.is_some()
        || snapshot.auto_compaction_failures > 0
        || snapshot.stale_turns_recovered > 0
}

/// 格式化恢复快照。
///
/// 参数:
/// - `snapshot`: 当前恢复快照
///
/// 返回:
/// - 可显示的恢复状态
pub(crate) fn format_recovery_snapshot(snapshot: &RecoverySnapshot) -> String {
    let mut parts = Vec::new();
    if let Some(record) = &snapshot.latest {
        parts.push(format!(
            "{} {}, {}",
            kind_label(&record.kind),
            status_label(&record.status),
            record.reason
        ));
        if record.retry_count > 0 {
            parts.push(format!("连续失败 {} 次", record.retry_count));
        }
        if let Some(checkpoint_id) = &record.checkpoint_id {
            parts.push(format!("安全检查点 {checkpoint_id}"));
        }
    }
    if snapshot.auto_compaction_blocked {
        parts.push("自动压缩已熔断".to_string());
    }
    if snapshot.stale_turns_recovered > 0 {
        parts.push(format!(
            "已恢复 stale 轮次 {} 个",
            snapshot.stale_turns_recovered
        ));
    }
    parts.join("，")
}

/// 恢复类型中文标签。
///
/// 参数:
/// - `kind`: 恢复类型
///
/// 返回:
/// - 中文标签
fn kind_label(kind: &FailureKind) -> &'static str {
    match kind {
        FailureKind::CompactionLlmFailed => "压缩模型失败",
        FailureKind::EmptySummary => "空压缩摘要",
        FailureKind::CompactionOverBudget => "压缩请求超预算",
        FailureKind::CompactionMirrorFailed => "压缩兼容镜像写入失败",
        FailureKind::ProviderOverflow => "Provider 上下文溢出",
        FailureKind::OverflowRetryFailed => "溢出重试失败",
        FailureKind::StaleRunningTurn => "陈旧运行轮次",
        FailureKind::ProjectionInvalid => "历史投影异常",
        FailureKind::SessionMemoryExtractionFailed => "会话记忆提取失败",
        FailureKind::SessionMemoryBoundaryInvalid => "会话记忆边界异常",
        FailureKind::SessionMemoryCompactFailed => "会话记忆压缩失败",
        FailureKind::ToolHistoryMissingResult => "工具历史缺失结果",
        FailureKind::ToolHistoryOrphanResult => "工具历史孤立结果",
        FailureKind::ToolHistoryDuplicateResult => "工具历史重复结果",
        FailureKind::ToolHistoryPendingStale => "工具历史陈旧挂起",
        FailureKind::ToolHistoryReplacementMissing => "工具历史替换缺失",
        FailureKind::ToolHistoryPromptOverBudget => "工具历史提示超预算",
    }
}

/// 恢复状态中文标签。
///
/// 参数:
/// - `status`: 恢复状态
///
/// 返回:
/// - 中文标签
fn status_label(status: &RecoveryStatus) -> &'static str {
    match status {
        RecoveryStatus::Observed => "已记录",
        RecoveryStatus::Recovering => "恢复中",
        RecoveryStatus::Reprojected => "已重投影",
        RecoveryStatus::Resolved => "已恢复",
        RecoveryStatus::Terminal => "终止",
    }
}
