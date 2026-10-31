use super::ReplRuntime;

/// 【终端】【面板操作】行内容与操作分离，不通过标题字符串判断按钮。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PanelAction {
    Todo,
    AgentsToggle,
    Agent(Option<usize>),
    QueueToggle,
    QueueItem(usize),
    QueueAction(super::queue_panel::QueuePanelAction),
}

impl ReplRuntime {
    /// 【用户队列】【命中映射】队列绘制和操作共享行与列，不把按钮间空白识别成操作。
    /// 参数: cols 为当前输入区域宽度；返回: 队列在沉底面板中的操作区域。
    pub(super) fn queue_panel_hits(&self, cols: usize) -> Vec<super::queue_panel_view::QueueHit> {
        let todos =
            if self.sidebar_visible(usize::from(super::viewport::TerminalSize::current().cols)) {
                &[][..]
            } else {
                self.transcript.latest_todo_items()
            };
        let offset = super::bottom_panel::render_panel_lines(
            todos,
            &[],
            &self.queued_control_commands(),
            &[],
            cols,
            self.todo_panel_compact,
            0,
        )
        .len();
        super::queue_panel_view::render_at(
            &self.queue_panel,
            &self.queued_items(),
            self.stream_active,
            cols,
        )
        .into_iter()
        .enumerate()
        .flat_map(|(row, entry)| {
            entry
                .hits
                .into_iter()
                .map(move |(columns, action)| super::queue_panel_view::QueueHit {
                    row: row + offset,
                    columns,
                    action,
                })
        })
        .collect()
    }
    /// 【终端】【面板命中】与面板渲染使用同一来源，计算标题和子代理行的操作映射。
    /// 参数: 无；返回: 原面板行号与操作。
    pub(super) fn panel_targets(&self) -> Vec<(usize, PanelAction)> {
        let mut targets = Vec::new();
        let sidebar = self.sidebar_visible(usize::from(self.viewport.size().cols));
        let todos = if sidebar {
            &[][..]
        } else {
            self.transcript.latest_todo_items()
        };
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
        let (_, agents) = if sidebar && !self.agent_panel.is_active() {
            (Vec::new(), Vec::new())
        } else {
            self.agent_panel
                .panel_layout(&self.transcript.subagent_overview(), 0)
        };
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
            PanelAction::QueueToggle => {
                if self.queue_panel.is_active() {
                    self.queue_panel.deactivate();
                } else {
                    self.queue_panel.activate(self.queue_len());
                    self.clamp_queue_panel();
                    self.agent_panel.deactivate();
                }
            }
            PanelAction::QueueItem(index) => {
                self.queue_panel.active = true;
                self.queue_panel.select(index);
                self.clamp_queue_panel();
                self.agent_panel.deactivate();
            }
            PanelAction::QueueAction(action) => {
                // 1. 【用户队列】【鼠标派发】进入既有编辑事件循环，空闲与流式共用相同保护逻辑
                let code = match action {
                    super::queue_panel::QueuePanelAction::MoveUp => {
                        crossterm::event::KeyCode::Char('k')
                    }
                    super::queue_panel::QueuePanelAction::MoveDown => {
                        crossterm::event::KeyCode::Char('j')
                    }
                    super::queue_panel::QueuePanelAction::Top => {
                        crossterm::event::KeyCode::Char('t')
                    }
                    super::queue_panel::QueuePanelAction::ToggleInsert => {
                        crossterm::event::KeyCode::Tab
                    }
                    super::queue_panel::QueuePanelAction::Delete => {
                        crossterm::event::KeyCode::Char('d')
                    }
                    super::queue_panel::QueuePanelAction::UndoDelete => {
                        crossterm::event::KeyCode::Char('u')
                    }
                    super::queue_panel::QueuePanelAction::Edit => crossterm::event::KeyCode::Enter,
                    super::queue_panel::QueuePanelAction::SendNow => {
                        crossterm::event::KeyCode::Char('s')
                    }
                    _ => crossterm::event::KeyCode::Esc,
                };
                self.return_input_event(crossterm::event::Event::Key(
                    crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE),
                ));
            }
        }
    }
}
