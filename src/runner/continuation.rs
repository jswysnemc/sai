/// runner continuation 触发原因。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ContinuationReason {
    ToolFollowUp,
    CompactionRetry,
    OverflowRetry,
    RecoveryResume,
}

/// runner continuation 描述。
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct RunnerContinuation {
    pub(crate) reason: ContinuationReason,
    pub(crate) turn_id: Option<String>,
    pub(crate) attempt: usize,
}

impl RunnerContinuation {
    /// 创建 runner continuation。
    ///
    /// 参数:
    /// - `reason`: continuation 触发原因
    ///
    /// 返回:
    /// - runner continuation
    pub(crate) fn new(reason: ContinuationReason) -> Self {
        Self {
            reason,
            turn_id: None,
            attempt: 0,
        }
    }

    /// 设置 continuation 所属 turn id。
    ///
    /// 参数:
    /// - `turn_id`: turn id
    ///
    /// 返回:
    /// - 更新后的 runner continuation
    pub(crate) fn with_turn_id(mut self, turn_id: impl Into<String>) -> Self {
        self.turn_id = Some(turn_id.into());
        self
    }

    /// 设置 continuation 尝试次数。
    ///
    /// 参数:
    /// - `attempt`: 尝试次数
    ///
    /// 返回:
    /// - 更新后的 runner continuation
    pub(crate) fn with_attempt(mut self, attempt: usize) -> Self {
        self.attempt = attempt;
        self
    }
}
