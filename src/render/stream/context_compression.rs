use super::*;

impl StreamRenderer {
    /// 【上下文】【压缩反馈】在流式终端显示摘要生成与执行状态，不输出完整参数
    /// 参数: preparing 表示摘要仍在生成；返回刷新结果
    pub(super) fn write_context_compression_progress(&mut self, preparing: bool) -> Result<()> {
        self.stop_command_preview()?;
        self.set_work_status(WorkStatus::Compacting, false)?;
        self.write_live_tool_status(
            tool_view::compression_progress_label(preparing),
            "run",
            false,
        )
    }

    /// 【上下文】【压缩反馈】提交压缩结果行并恢复等待状态
    /// 参数: ok 为执行结果，output 为工具回执；返回刷新结果
    pub(super) fn write_context_compression_result(
        &mut self,
        ok: bool,
        output: &str,
    ) -> Result<()> {
        self.set_work_status(WorkStatus::WaitingResponse, false)?;
        self.write_live_tool_status(
            &tool_view::compression_result_label(ok, output),
            if ok { "ok" } else { "err" },
            true,
        )?;
        self.resume_work_spinner()
    }
}
