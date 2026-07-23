use super::*;

impl StreamRenderer {
    /// 写入单行工具状态。
    ///
    /// 参数:
    /// - `name`: 工具展示标签
    /// - `status`: 工具状态，取值为 arg、run、ok 或 err
    /// - `final_line`: 是否结束当前状态行
    ///
    /// 返回:
    /// - 写入是否成功
    pub(super) fn write_live_tool_status(
        &mut self,
        name: &str,
        status: &str,
        final_line: bool,
    ) -> Result<()> {
        if !self.live_tool_status.is_active() {
            self.end_active_stream_line()?;
            self.finalize_reasoning_summary()?;
        }
        self.live_tool_status
            .write(self.summary.display_tool_name(name), status, final_line)
    }

    /// 结束当前单行工具状态。
    ///
    /// 返回:
    /// - 写入是否成功
    pub(super) fn finish_live_tool_status(&mut self) -> Result<()> {
        self.live_tool_status.finish()
    }

    /// 清除当前单行工具状态。
    ///
    /// 返回:
    /// - 写入是否成功
    pub(super) fn clear_live_tool_status(&mut self) -> Result<()> {
        self.live_tool_status.clear()
    }
}
