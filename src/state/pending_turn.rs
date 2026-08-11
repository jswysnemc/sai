use super::partial_turn_sink::PartialTurnSink;
use super::StateStore;
use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct PendingTurnGuard {
    state: StateStore,
    turn_id: String,
    settled: bool,
    cancel_requested: Option<Arc<AtomicBool>>,
    partial: PartialTurnSink,
}

impl PendingTurnGuard {
    /// 创建待完成轮次守卫。
    ///
    /// 参数:
    /// - `state`: 状态存储
    /// - `turn_id`: 当前轮唯一标识
    /// - `partial`: 流式增量累积句柄，与事件回调闭包共享
    ///
    /// 返回:
    /// - 待完成轮次守卫
    pub fn new(state: StateStore, turn_id: String, partial: PartialTurnSink) -> Self {
        Self {
            state,
            turn_id,
            settled: false,
            cancel_requested: None,
            partial,
        }
    }

    /// 绑定用户停止请求标志。
    ///
    /// 绑定后，提前结束时按标志判定归因：置位记为用户中断，否则记为失败。
    /// 不绑定则一律按失败记录，避免把上游报错归因给用户。
    ///
    /// 参数:
    /// - `cancel_requested`: 与 Agent 共享的停止标志
    ///
    /// 返回:
    /// - 已绑定停止标志的守卫
    pub fn with_cancel_flag(mut self, cancel_requested: Arc<AtomicBool>) -> Self {
        self.cancel_requested = Some(cancel_requested);
        self
    }

    /// 判断本轮的提前结束是否源自用户主动停止。
    ///
    /// 返回:
    /// - 已绑定标志且标志置位时返回 true
    fn cancelled_by_user(&self) -> bool {
        self.cancel_requested
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
    }

    /// 完成当前轮次并关闭守卫。
    ///
    /// 参数:
    /// - `content`: 助手回复
    /// - `reasoning`: 可选推理内容
    ///
    /// 返回:
    /// - 完成是否成功
    pub fn complete(mut self, content: &str, reasoning: Option<&str>) -> Result<()> {
        self.state
            .complete_turn(&self.turn_id, content, reasoning)?;
        self.settled = true;
        Ok(())
    }

    /// 将当前轮次标记为失败，保留真实错误原因。
    ///
    /// 参数:
    /// - `error`: 失败原因
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn fail(mut self, error: &str) -> Result<()> {
        self.fail_in_place(error)
    }

    /// 保留守卫所有权地写入失败终态。
    ///
    /// 一轮里有多个可能提前返回的准备步骤，每一步都消耗守卫就无法继续往下走；
    /// 这里只落终态不交出所有权，后续步骤仍可正常使用同一个守卫。
    ///
    /// 参数:
    /// - `error`: 失败原因
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn fail_in_place(&mut self, error: &str) -> Result<()> {
        if !self.settled {
            self.persist_failure(error)?;
            self.settled = true;
        }
        Ok(())
    }

    /// 将当前轮次标记为用户中断。
    ///
    /// 返回:
    /// - 中断是否成功
    #[allow(dead_code)]
    pub fn interrupt(&mut self) -> Result<()> {
        if !self.settled {
            self.persist_interruption()?;
            self.settled = true;
        }
        Ok(())
    }
}

impl PendingTurnGuard {
    /// 将当前轮次和已经产生的部分内容保存为中断状态。
    ///
    /// 返回:
    /// - 写入是否成功
    fn persist_interruption(&self) -> Result<()> {
        let partial = self.partial.snapshot();
        self.state.interrupt_turn(
            &self.turn_id,
            &partial.content,
            (!partial.reasoning.trim().is_empty()).then_some(partial.reasoning.as_str()),
        )
    }

    /// 将当前轮次保存为失败状态。
    ///
    /// 参数:
    /// - `error`: 失败原因
    ///
    /// 返回:
    /// - 写入是否成功
    fn persist_failure(&self, error: &str) -> Result<()> {
        let partial = self.partial.snapshot();
        self.state.fail_turn(
            &self.turn_id,
            &partial.content,
            (!partial.reasoning.trim().is_empty()).then_some(partial.reasoning.as_str()),
            error,
        )
    }
}

impl Drop for PendingTurnGuard {
    fn drop(&mut self) {
        if self.settled {
            return;
        }
        // 提前结束时按停止标志归因：用户按下停止才算中断。
        // 无条件记成中断会让上游报错、内部短路都显示成"用户已中断"，
        // 真实原因反而被覆盖
        let _ = if self.cancelled_by_user() {
            self.persist_interruption()
        } else {
            self.persist_failure(crate::i18n::text(
                "the turn ended before a terminal state was recorded",
                "本轮在写入终态前结束",
            ))
        };
    }
}
