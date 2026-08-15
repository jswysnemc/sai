use super::*;

impl Agent {
    /// 构造当前轮完整请求消息。
    ///
    /// 参数:
    /// - `turn_id`: 当前运行中轮次标识
    /// - `input`: 当前用户输入
    /// - `image_urls`: 图片 data URL 列表
    /// - `association_prompt`: 可选关联记忆上下文
    /// - `auto_meme_reminder`: 可选自动表情包提醒
    ///
    /// 返回:
    /// - 当前轮请求消息列表
    pub(super) fn chat_messages_for_turn(
        &mut self,
        turn_id: &str,
        input: &str,
        image_urls: &[String],
        association_prompt: Option<&str>,
        auto_meme_reminder: Option<&str>,
    ) -> Result<Vec<ChatMessage>> {
        let base_projection = self.chat_base_context_projection(Some(turn_id))?;
        let projection = project_provider_turn_from_base_projection(
            base_projection,
            input,
            image_urls,
            association_prompt,
            auto_meme_reminder,
            0,
            self.context_char_budget,
        );
        self.last_dynamic_sources = projection.dynamic_sources.clone();
        self.state
            .enforce_provider_projection(Some(turn_id), &projection)?;
        if let Some(provider_user_content) = projection.provider_user_content.as_deref() {
            self.state
                .set_provider_user_content(turn_id, provider_user_content)?;
        }
        Ok(projection.messages)
    }

    /// 构造 provider base context 投影。
    ///
    /// 参数:
    /// - `exclude_turn_id`: 需要从历史投影中排除的当前运行中轮次
    ///
    /// 返回:
    /// - provider base context 投影
    pub(super) fn chat_base_context_projection(
        &self,
        exclude_turn_id: Option<&str>,
    ) -> Result<ProjectedBaseContext> {
        let goal_context = self
            .state
            .goal()?
            .map(|goal| crate::goal::system_context(&goal));
        let session_goal_context = goal_context.unwrap_or_default();
        let projected_history = self.state.project_history(exclude_turn_id)?;
        let compaction_summary_context = projected_history
            .checkpoint_context
            .or(self.state.compaction_summary_context()?);
        let last_auto_meme_reminder = memes::last_auto_meme_reminder(&self.config, &self.paths)?;
        let selected_model = selected_model_label(&self.config)?;
        let snapshot = RuntimeContextSnapshot::capture(selected_model.as_deref());
        let runtime_update = context_state_update(
            &snapshot,
            self.mode(),
            compaction_summary_context.as_deref(),
            &projected_history.messages,
            self.config.prompt_sections.mode_reminder,
        )?;
        let goal_update = context_resources::context_resource_update(
            "goal",
            &session_goal_context,
            compaction_summary_context.as_deref(),
            &projected_history.messages,
        )?;
        let meme_update = context_resources::context_resource_update(
            "last_auto_meme",
            last_auto_meme_reminder.as_deref().unwrap_or_default(),
            compaction_summary_context.as_deref(),
            &projected_history.messages,
        )?;
        let state_update =
            context_resources::combine_context_updates([runtime_update, goal_update, meme_update]);
        let epoch = self
            .state
            .context_epoch_projection(&self.base_system_prompt)?;
        Ok(project_provider_base_context_projection(
            &epoch.baseline,
            compaction_summary_context.as_deref(),
            projected_history.messages,
            state_update.as_deref(),
        ))
    }
}
