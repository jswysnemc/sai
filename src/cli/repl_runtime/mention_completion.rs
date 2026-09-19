use super::ReplRuntime;
use crate::cli::repl_mentions::MentionSuggestion;

impl ReplRuntime {
    /// 【终端】【补全候选】请求与当前输入一致的候选，不等待文件系统
    /// 参数: input/cursor 为草稿；返回已经完成的候选列表
    pub(in crate::cli) fn mention_candidates(
        &self,
        input: &str,
        cursor: usize,
    ) -> Vec<MentionSuggestion> {
        let mut completion = self
            .mention_completion
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if self
            .panels_dismissed
            .as_ref()
            .is_some_and(|(text, position)| text == input && *position == cursor)
        {
            completion.cancel();
            return Vec::new();
        }
        completion.query(input, cursor, &self.mention_skills)
    }

    /// 返回当前引用查询是否尚未结束，无参数
    pub(in crate::cli) fn mention_completion_pending(&self) -> bool {
        self.mention_completion
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .pending()
    }

    /// 收到后台结果时刷新流式输入框，无参数；返回绘制结果
    pub(super) fn poll_mention_completion(&mut self) -> anyhow::Result<()> {
        let changed = self
            .mention_completion
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .poll();
        if changed {
            self.redraw_stream_composer()?;
        }
        Ok(())
    }
}
