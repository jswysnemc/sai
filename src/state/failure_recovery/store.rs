use super::{FailureKind, RecoverySnapshot, RecoveryStatus};
use crate::state::{failure_recovery, ContextEpochProjection, StateStore};
use anyhow::Result;

impl StateStore {
    /// 读取当前会话恢复快照。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 恢复快照
    pub fn recovery_snapshot(&self) -> Result<RecoverySnapshot> {
        failure_recovery::recovery_snapshot(&self.conv_db, &self.session_id)
    }

    /// 判断当前会话是否允许自动压缩。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否允许自动压缩
    pub fn should_attempt_auto_compaction(&self) -> Result<bool> {
        let snapshot = self.recovery_snapshot()?;
        Ok(failure_recovery::should_attempt_auto_compaction(&snapshot))
    }

    /// 记录自动压缩失败。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮次标识
    /// - `kind`: 失败类型
    /// - `reason`: 失败原因
    /// - `context_chars`: 当前上下文字符数
    /// - `context_limit_chars`: 上下文预算字符数
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn record_auto_compaction_failure(
        &self,
        turn_id: Option<&str>,
        kind: FailureKind,
        reason: &str,
        context_chars: usize,
        context_limit_chars: usize,
    ) -> Result<()> {
        let snapshot = self.recovery_snapshot()?;
        let retry_count = failure_recovery::next_auto_compaction_retry_count(&snapshot);
        self.record_recovery_failure(
            turn_id,
            kind,
            RecoveryStatus::Observed,
            reason,
            retry_count,
            context_chars,
            context_limit_chars,
        )?;
        Ok(())
    }

    /// 记录手动压缩失败。
    ///
    /// 参数:
    /// - `kind`: 失败类型
    /// - `reason`: 失败原因
    /// - `context_chars`: 当前上下文字符数
    /// - `context_limit_chars`: 上下文预算字符数
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn record_manual_compaction_failure(
        &self,
        kind: FailureKind,
        reason: &str,
        context_chars: usize,
        context_limit_chars: usize,
    ) -> Result<()> {
        self.record_recovery_failure(
            None,
            kind,
            RecoveryStatus::Observed,
            reason,
            0,
            context_chars,
            context_limit_chars,
        )?;
        Ok(())
    }

    /// 记录 provider overflow 恢复事件。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮次标识
    /// - `kind`: 恢复类型
    /// - `status`: 恢复状态
    /// - `reason`: 恢复原因
    /// - `context_chars`: 当前上下文字符数
    /// - `context_limit_chars`: 上下文预算字符数
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn record_provider_overflow_recovery(
        &self,
        turn_id: Option<&str>,
        kind: FailureKind,
        status: RecoveryStatus,
        reason: &str,
        context_chars: usize,
        context_limit_chars: usize,
    ) -> Result<()> {
        self.record_recovery_failure(
            turn_id,
            kind,
            status,
            reason,
            1,
            context_chars,
            context_limit_chars,
        )?;
        Ok(())
    }

    /// 标记当前会话活跃压缩失败已恢复。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 更新是否成功
    pub fn resolve_active_compaction_failures(&self) -> Result<()> {
        failure_recovery::resolve_active_compaction_failures(&self.conv_db, &self.session_id)?;
        Ok(())
    }

    /// 记录 Context Epoch 投影失败。
    ///
    /// 参数:
    /// - `result`: Context Epoch 投影结果
    ///
    /// 返回:
    /// - 记录是否成功
    pub(in crate::state) fn record_context_epoch_projection_result(
        &self,
        result: &Result<ContextEpochProjection>,
    ) -> Result<()> {
        if let Err(error) = result {
            self.record_recovery_failure(
                None,
                FailureKind::ProjectionInvalid,
                RecoveryStatus::Terminal,
                &format!("Context Epoch 投影失败: {error}"),
                0,
                0,
                0,
            )?;
        }
        Ok(())
    }

    /// 写入恢复记录。
    ///
    /// 参数:
    /// - `turn_id`: 可选轮次标识
    /// - `kind`: 失败类型
    /// - `status`: 恢复状态
    /// - `reason`: 原因
    /// - `retry_count`: 连续重试次数
    /// - `context_chars`: 当前上下文字符数
    /// - `context_limit_chars`: 上下文预算字符数
    ///
    /// 返回:
    /// - 写入是否成功
    pub(in crate::state) fn record_recovery_failure(
        &self,
        turn_id: Option<&str>,
        kind: FailureKind,
        status: RecoveryStatus,
        reason: &str,
        retry_count: usize,
        context_chars: usize,
        context_limit_chars: usize,
    ) -> Result<()> {
        let checkpoint_id = failure_recovery::latest_checkpoint_id(&self.conv_db)?;
        failure_recovery::record_failure(
            &self.conv_db,
            failure_recovery::NewRecoveryRecord {
                session_id: self.session_id.clone(),
                turn_id: turn_id.map(str::to_string),
                kind,
                status,
                reason: reason.to_string(),
                retry_count,
                checkpoint_id,
                context_chars,
                context_limit_chars,
            },
        )?;
        Ok(())
    }
}
