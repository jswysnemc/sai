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
        let label = self.summary.display_tool_name(name).to_string();
        if !final_line {
            if !self.live_tool_status.is_active() {
                self.end_active_stream_line()?;
                self.finalize_reasoning_summary()?;
            }
            self.live_tool_status.arm();
            // 终端里用底行扫光画出进行中的工具；非终端退回单行静态状态
            if crate::render::wait_spinner::WaitSpinner::supported() && !self.plain {
                return self.show_tool_shimmer(&label);
            }
            return self.live_tool_status.write(&label, status, false);
        }
        let finished = crate::render::tool_event_line::tool_event_text(&label, status);
        if let Some(spinner) = self.wait_spinner.as_ref() {
            spinner.commit_and_shift(&finished)?;
            self.live_tool_status.disarm();
            if let Some(work) = self.work_status {
                spinner.set_phase(work.localized_label());
            }
            return Ok(());
        }
        self.live_tool_status.write(&label, status, true)
    }

    /// 用底行扫光显示进行中的工具名。
    ///
    /// 参数:
    /// - `label`: 工具展示标签
    ///
    /// 返回:
    /// - 是否成功接上动效
    fn show_tool_shimmer(&mut self, label: &str) -> Result<()> {
        if self.command_preview.is_active() {
            return self.live_tool_status.write(label, "run", false);
        }
        let started_at = *self
            .work_started
            .get_or_insert_with(crate::render::activity_animation::activity_started_at);
        if let Some(spinner) = self.wait_spinner.as_ref() {
            spinner.set_phase(label);
            spinner.set_sub_phase(None);
            return Ok(());
        }
        self.hide_cursor()?;
        self.wait_spinner = Some(crate::render::wait_spinner::WaitSpinner::start_with_clock(
            label.to_string(),
            None,
            started_at,
        ));
        Ok(())
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
