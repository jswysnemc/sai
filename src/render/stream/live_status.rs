use super::*;

impl StreamRenderer {
    /// 写入单行工具状态。
    ///
    /// 进入前先停掉 WaitSpinner：底行同一时刻只允许一个所有者。
    /// spinner 线程按帧用绝对定位重画锚点行，而本状态行写在当前光标行，
    /// 光标又会被 spinner 线程随时挪走——两者并存时相互覆盖，
    /// 底行在 Working 与工具状态之间来回闪。spinner 停止时会把光标
    /// 归位到锚点行首，工具状态行因此正好接管同一行。
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
        self.stop_waiting()?;
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
