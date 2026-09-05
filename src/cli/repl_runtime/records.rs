use super::*;

impl ReplRuntime {
    /// 【终端】【输入回显】记录携带原子块来源的提交并同步历史视图。
    ///
    /// 参数: `mode` 为提交模式，`echo` 为完整正文与原子块元数据
    /// 返回: 终端同步结果
    pub(in crate::cli) fn record_input(
        &mut self,
        mode: AgentMode,
        echo: crate::render::input_atom::InputEcho,
    ) -> Result<()> {
        self.transcript
            .push_user_input(layout::transcript_mode(mode), echo);
        self.sync_transcript(false)
    }

    /// 记录用户输入并立即插入 source-backed 历史。
    ///
    /// 参数:
    /// - `mode`: 用户提交时的 REPL 模式
    /// - `text`: 回显正文（粘贴长文本应已展开）
    /// - `fold`: 仅粘贴长文本为 true，启用思考式折叠
    ///
    /// 返回:
    /// - 操作是否成功
    pub(in crate::cli) fn record_user(
        &mut self,
        mode: AgentMode,
        text: String,
        fold: bool,
    ) -> Result<()> {
        self.transcript
            .push_user_echo_with_fold(layout::transcript_mode(mode), text, fold);
        self.sync_transcript(false)
    }

    /// 将已保存的会话历史与压缩摘要渲染到当前 TUI transcript。
    ///
    /// 参数:
    /// - `turns`: 按时间顺序排列的历史轮次
    /// - `compaction`: 最新压缩摘要
    ///
    /// 返回:
    /// - transcript 同步结果
    pub(in crate::cli) fn record_history_with_compaction(
        &mut self,
        turns: &[SessionTimelineTurn],
        compaction: Option<&SessionTimelineCompaction>,
    ) -> Result<()> {
        history::append_timeline_with_compaction(&mut self.transcript, turns, compaction);
        self.sync_transcript(false)
    }

    /// 记录控制命令、系统提示或错误信息。
    ///
    /// 参数:
    /// - `text`: 原始消息文本
    ///
    /// 返回:
    /// - 操作是否成功
    pub(in crate::cli) fn record_meta(&mut self, text: String) -> Result<()> {
        self.transcript.push_meta(text);
        self.sync_transcript(false)
    }

    /// 记录轮次失败或中断提示，带失败专属样式。
    ///
    /// 参数:
    /// - `text`: 失败说明
    ///
    /// 返回:
    /// - transcript 同步结果
    pub(in crate::cli) fn record_failure(&mut self, text: String) -> Result<()> {
        self.transcript.push_failure(text);
        self.sync_transcript(false)
    }

    /// 记录等待用户处理的权限事件。
    ///
    /// 参数:
    /// - `request`: 权限请求
    ///
    /// 返回:
    /// - transcript 同步结果
    pub(in crate::cli) fn record_permission_request(
        &mut self,
        request: crate::permission::PermissionRequest,
    ) -> Result<()> {
        self.transcript.push_permission_request(request);
        self.sync_transcript(false)
    }

    /// 更新 transcript 中权限事件的最终决定。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    /// - `decision`: 用户决定
    ///
    /// 返回:
    /// - transcript 同步结果
    pub(in crate::cli) fn resolve_permission(
        &mut self,
        request_id: &str,
        decision: crate::permission::PermissionDecision,
    ) -> Result<()> {
        self.transcript.resolve_permission(request_id, decision);
        self.sync_transcript(false)
    }

    /// 更新权限事件中的内联拒绝回复草稿。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    /// - `draft`: 回复草稿；空值表示返回权限选择
    ///
    /// 返回:
    /// - transcript 同步结果
    pub(in crate::cli) fn update_permission_reply(
        &mut self,
        request_id: &str,
        draft: Option<String>,
    ) -> Result<()> {
        self.transcript
            .set_permission_reply_draft(request_id, draft);
        self.sync_transcript(false)
    }

    /// 更新权限事件中的当前高亮选项。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    /// - `selected`: 高亮选项
    ///
    /// 返回:
    /// - transcript 同步结果
    pub(in crate::cli) fn update_permission_choice(
        &mut self,
        request_id: &str,
        selected: crate::render::PermissionChoice,
    ) -> Result<()> {
        self.transcript.set_permission_choice(request_id, selected);
        self.sync_transcript(false)
    }

    /// 权限交互开始时暂停工作动效，避免遮挡审计选择。
    ///
    /// 返回:
    /// - transcript 同步结果
    pub(in crate::cli) fn pause_for_permission_prompt(&mut self) -> Result<()> {
        self.next_live_refresh = None;
        self.live_sync_pending = false;
        self.transcript.clear_work_status();
        self.transcript.finalize_live_tail();
        self.sync_transcript(false)
    }

    /// 记录本地 Shell 命令与输出。
    ///
    /// 参数:
    /// - `command`: Shell 命令
    /// - `output`: 命令输出
    /// - `exit_code`: 可选退出码
    ///
    /// 返回:
    /// - 同步 transcript 是否成功
    pub(in crate::cli) fn record_shell(
        &mut self,
        command: String,
        output: String,
        exit_code: Option<i32>,
    ) -> Result<()> {
        self.transcript.push_shell(command, output, exit_code);
        self.sync_transcript(false)
    }

    /// 进入 `!` Shell 命令的等待状态并显示实时耗时。
    ///
    /// 命令在 REPL 主循环里同步等待，没有工作状态行的话界面会完全静止，
    /// 用户无法区分「正在跑」和「卡死了」。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 操作是否成功
    pub(in crate::cli) fn begin_shell_status(&mut self) -> Result<()> {
        self.transcript
            .set_work_status(crate::render::work_status::WorkStatus::WaitingToRun);
        // 立刻排一帧，让状态行在命令启动前就画出来
        self.next_live_refresh = Some(Instant::now());
        self.sync_transcript(false)
    }

    /// 结束 `!` Shell 命令的等待状态。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 操作是否成功
    pub(in crate::cli) fn end_shell_status(&mut self) -> Result<()> {
        self.transcript.clear_work_status();
        self.next_live_refresh = None;
        self.sync_transcript(false)
    }

    /// transcript 当前是否还挂着工作状态行。
    ///
    /// 返回:
    /// - 存在工作状态为真
    #[cfg(test)]
    pub(in crate::cli) fn transcript_has_work_status(&self) -> bool {
        self.transcript.has_work_status()
    }

    /// 记录 REPL 启动欢迎面板。
    ///
    /// 参数:
    /// - `version`: 当前程序版本
    /// - `model`: 当前模型名称
    /// - `directory`: 当前工作目录
    /// - `permissions`: 当前权限模式
    ///
    /// 返回:
    /// - 操作是否成功
    pub(in crate::cli) fn record_welcome(
        &mut self,
        version: String,
        model: String,
        directory: String,
        permissions: String,
    ) -> Result<()> {
        self.transcript.push_welcome(WelcomeCell {
            version,
            model,
            directory,
            permissions,
        });
        self.sync_transcript(false)
    }

    /// 在流结束后收敛 source，并修复所有 stream-time reflow。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 操作是否成功
    pub(in crate::cli) fn finish_stream(&mut self) -> Result<()> {
        self.next_live_refresh = None;
        self.live_sync_pending = false;
        // 轮次结束：斜杠面板恢复全部命令可选
        self.stream_active = false;
        self.transcript.finalize_live_tail();
        self.transcript.clear_work_status();
        if self.reflow.take_stream_finish_reflow_needed() {
            self.reflow.schedule_immediate();
            self.maybe_reflow_due(false)?;
            return Ok(());
        }
        self.sync_transcript(false)
    }
}
