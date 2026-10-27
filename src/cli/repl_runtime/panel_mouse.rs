use super::ReplRuntime;

/// 【终端】【面板操作】行内容与操作分离，不通过标题字符串判断按钮。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PanelAction {
    Todo,
    AgentsToggle,
    Agent(Option<usize>),
}

impl ReplRuntime {
    /// 【终端】【面板命中】与面板渲染使用同一来源，计算标题和子代理行的操作映射。
    /// 参数: 无；返回: 原面板行号与操作。
    pub(super) fn panel_targets(&self) -> Vec<(usize, PanelAction)> {
        let mut targets = Vec::new();
        let todos = self.transcript.latest_todo_items();
        if todos
            .iter()
            .any(|item| matches!(item.status.as_str(), "pending" | "in_progress"))
        {
            targets.push((0, PanelAction::Todo));
        }
        let queue = self
            .queue_panel
            .panel_lines(&self.queued_items(), self.stream_active);
        let offset = super::bottom_panel::render_panel_lines(
            todos,
            &queue,
            &self.queued_control_commands(),
            &[],
            usize::from(self.viewport.size().cols),
            self.todo_panel_compact,
            0,
        )
        .len();
        let (_, agents) = self
            .agent_panel
            .panel_layout(&self.transcript.subagent_overview(), 0);
        targets.extend(
            agents
                .into_iter()
                .filter(|(_, action)| match action {
                    PanelAction::Agent(Some(index)) => self.transcript.can_view_subagent(*index),
                    _ => true,
                })
                .map(|(row, action)| (offset + row, action)),
        );
        targets
    }

    /// 【终端】【面板命中】仅匹配屏幕上真实可见文字，不响应裁掉的标题或行外空白。
    /// 参数: `column` 与 `row` 为屏幕坐标；返回: 对应操作。
    pub(super) fn panel_target_at(&self, column: u16, row: u16) -> Option<PanelAction> {
        self.last_composer_signature
            .as_ref()?
            .panel_action_at(column, row)
    }

    /// 【终端】【面板悬停】记录指针位置，仅在命中对象变化时触发重绘。
    /// 参数: `column` 与 `row` 为屏幕坐标；返回: 悬停反馈是否变化。
    pub(super) fn move_panel_pointer(&mut self, column: u16, row: u16) -> bool {
        let before = self
            .panel_pointer
            .and_then(|(column, row)| self.panel_target_at(column, row));
        self.panel_pointer = Some((column, row));
        before != self.panel_target_at(column, row)
    }

    /// 【终端】【面板点击】待办切换紧凑模式，子代理标题展开列表，条目进入对应会话。
    /// 参数: `action` 为已验证命中的操作；返回: 无。
    pub(super) fn apply_panel_action(&mut self, action: PanelAction) {
        match action {
            PanelAction::Todo => {
                self.toggle_todo_panel_compact();
            }
            PanelAction::AgentsToggle => {
                if self.agent_panel.is_active() {
                    self.agent_panel.deactivate();
                } else {
                    self.agent_panel
                        .activate(&self.transcript.subagent_overview());
                    self.queue_panel.deactivate();
                }
            }
            PanelAction::Agent(choice) => {
                self.agent_panel.deactivate();
                match choice {
                    None => {
                        self.transcript.exit_subagent_view();
                    }
                    Some(index) => {
                        self.transcript.enter_subagent_view(index);
                    }
                }
            }
        }
    }
}
