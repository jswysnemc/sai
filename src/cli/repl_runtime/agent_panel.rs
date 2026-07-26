use crate::i18n::text as t;
use crate::render::status_style::color_status;
use crate::render::transcript::SubagentOverviewEntry;
use crossterm::event::KeyCode;

/// 底部主/子 agent 切换面板的交互状态。
#[derive(Default)]
pub(super) struct AgentPanelState {
    /// 是否处于选择焦点态（↓ 进入，Esc 退出）
    active: bool,
    /// 当前高亮下标：0 为主 agent，1.. 对应子 agent 条目
    selected: usize,
}

/// 面板按键处理结果。
#[derive(Debug, Eq, PartialEq)]
pub(super) enum AgentPanelAction {
    /// 按键与面板无关，交回常规输入处理
    Ignored,
    /// 按键已被面板消费，仅需重绘底部
    Consumed,
    /// 用户确认选择：`None` 表示主 agent，`Some(cell_index)` 为子 agent
    Apply(Option<usize>),
}

impl AgentPanelState {
    /// 返回面板是否处于焦点态。
    ///
    /// 返回:
    /// - 焦点态标志
    pub(super) fn is_active(&self) -> bool {
        self.active
    }

    /// 尝试进入焦点态（存在子 agent 时才生效）。
    ///
    /// 参数:
    /// - `entries`: 当前子 agent 概览
    ///
    /// 返回:
    /// - 是否进入焦点态
    pub(super) fn activate(&mut self, entries: &[SubagentOverviewEntry]) -> bool {
        if entries.is_empty() {
            return false;
        }
        self.active = true;
        // 默认高亮当前正在查看的子 agent；主视图下高亮主 agent
        self.selected = entries
            .iter()
            .position(|entry| entry.viewing)
            .map(|position| position + 1)
            .unwrap_or(0);
        true
    }

    /// 退出焦点态。
    ///
    /// 返回:
    /// - 无
    pub(super) fn deactivate(&mut self) {
        self.active = false;
    }

    /// 处理焦点态下的按键。
    ///
    /// 参数:
    /// - `code`: 键码
    /// - `entries`: 当前子 agent 概览
    ///
    /// 返回:
    /// - 面板动作
    pub(super) fn handle_key(
        &mut self,
        code: KeyCode,
        entries: &[SubagentOverviewEntry],
    ) -> AgentPanelAction {
        if !self.active {
            return AgentPanelAction::Ignored;
        }
        let total = entries.len() + 1;
        match code {
            KeyCode::Up => {
                self.selected = self.selected.checked_sub(1).unwrap_or(total - 1);
                AgentPanelAction::Consumed
            }
            KeyCode::Down => {
                self.selected = (self.selected + 1) % total;
                AgentPanelAction::Consumed
            }
            KeyCode::Enter => {
                let choice = if self.selected == 0 {
                    None
                } else {
                    entries.get(self.selected - 1).map(|entry| entry.cell_index)
                };
                self.active = false;
                AgentPanelAction::Apply(choice)
            }
            KeyCode::Esc => {
                self.active = false;
                AgentPanelAction::Consumed
            }
            // 其他键：退出面板并交回常规输入处理
            _ => {
                self.active = false;
                AgentPanelAction::Ignored
            }
        }
    }

    /// 渲染面板行（未截断，由沉底面板统一截断到终端宽度）。
    ///
    /// 参数:
    /// - `entries`: 当前子 agent 概览
    ///
    /// 返回:
    /// - 面板 ANSI 行；无子 agent 时为空
    pub(super) fn panel_lines(&self, entries: &[SubagentOverviewEntry]) -> Vec<String> {
        if entries.is_empty() {
            return Vec::new();
        }
        if !self.active {
            // 非焦点态：单行提示，↓ 进入切换
            let running = entries.iter().filter(|entry| entry.running).count();
            return vec![format!(
                "\x1b[2m• {}: {} ({}) · ↓ {}\x1b[0m",
                t("agents", "智能体"),
                entries.len(),
                format!("{running} {}", t("running", "运行中")),
                t("switch", "切换")
            )];
        }
        let mut lines = vec![format!(
            "\x1b[2m• {} · ↑↓ {} · Enter {} · Esc {}\x1b[0m",
            t("agents", "智能体"),
            t("select", "选择"),
            t("view", "查看"),
            t("back", "返回")
        )];
        lines.push(selection_line(
            self.selected == 0,
            &format!("{} {}", t("main agent", "主智能体"), t("(overview)", "(总览)")),
            None,
        ));
        for (position, entry) in entries.iter().enumerate() {
            let viewing_suffix = if entry.viewing {
                format!(" \x1b[2m({})\x1b[0m", t("viewing", "查看中"))
            } else {
                String::new()
            };
            lines.push(selection_line(
                self.selected == position + 1,
                &format!(
                    "{} {}{viewing_suffix}",
                    entry.label,
                    color_status(entry.status)
                ),
                Some(entry.status),
            ));
        }
        lines
    }
}

impl super::ReplRuntime {
    /// 返回底部 agent 面板是否处于焦点态。
    ///
    /// 返回:
    /// - 焦点态标志
    pub(in crate::cli) fn agent_panel_active(&self) -> bool {
        self.agent_panel.is_active()
    }

    /// 面板按键核心处理（不负责重绘输入框）。
    ///
    /// 参数:
    /// - `code`: 键码
    ///
    /// 返回:
    /// - 按键被面板消费时返回 true
    fn agent_panel_key_inner(&mut self, code: KeyCode) -> anyhow::Result<bool> {
        let entries = self.transcript.subagent_overview();
        if self.agent_panel.is_active() {
            return match self.agent_panel.handle_key(code, &entries) {
                AgentPanelAction::Consumed => Ok(true),
                AgentPanelAction::Apply(choice) => {
                    // 主 agent：返回主会话视图；子 agent：整体切换到其会话时间线
                    let changed = match choice {
                        None => self.transcript.exit_subagent_view(),
                        Some(cell_index) => self.transcript.enter_subagent_view(cell_index),
                    };
                    if changed {
                        // 视图整体切换：从 source 全量重放
                        self.redraw()?;
                    }
                    Ok(true)
                }
                AgentPanelAction::Ignored => Ok(false),
            };
        }
        if code == KeyCode::Down && self.agent_panel.activate(&entries) {
            return Ok(true);
        }
        Ok(false)
    }

    /// 处理流式阶段的 agent 面板按键（消费后重绘底部输入框）。
    ///
    /// 参数:
    /// - `code`: 键码
    ///
    /// 返回:
    /// - 按键被面板消费时返回 true
    pub(in crate::cli) fn handle_agent_panel_key(
        &mut self,
        code: KeyCode,
    ) -> anyhow::Result<bool> {
        let was_active = self.agent_panel.is_active();
        let consumed = self.agent_panel_key_inner(code)?;
        // 面板退出（含转交普通处理）也要重绘一次，去掉列表行
        if consumed || was_active {
            self.redraw_stream_composer()?;
        }
        Ok(consumed)
    }

    /// 处理空闲输入阶段的 agent 面板按键（重绘交给输入主循环）。
    ///
    /// 参数:
    /// - `code`: 键码
    ///
    /// 返回:
    /// - 按键被面板消费时返回 true
    pub(in crate::cli) fn handle_agent_panel_idle_key(
        &mut self,
        code: KeyCode,
    ) -> anyhow::Result<bool> {
        self.agent_panel_key_inner(code)
    }
}

/// 渲染单行选择条目。
///
/// 参数:
/// - `selected`: 是否为当前高亮
/// - `content`: 条目内容（可含 ANSI）
/// - `status`: 可选状态键（仅用于语义，占位保持签名稳定）
///
/// 返回:
/// - ANSI 行
fn selection_line(selected: bool, content: &str, status: Option<&str>) -> String {
    let _ = status;
    if selected {
        format!("\x1b[1m\x1b[36m  ❯ {content}\x1b[0m")
    } else {
        format!("\x1b[2m    {content}\x1b[0m")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造测试概览条目。
    fn entry(cell_index: usize, label: &str, running: bool) -> SubagentOverviewEntry {
        SubagentOverviewEntry {
            cell_index,
            label: label.to_string(),
            status: if running { "run" } else { "ok" },
            running,
            viewing: false,
        }
    }

    #[test]
    fn activate_requires_subagents() {
        let mut panel = AgentPanelState::default();
        assert!(!panel.activate(&[]));
        assert!(panel.activate(&[entry(2, "检查项目", true)]));
        assert!(panel.is_active());
    }

    #[test]
    fn keys_cycle_and_apply_selection() {
        let mut panel = AgentPanelState::default();
        let entries = vec![entry(2, "one", true), entry(5, "two", false)];
        panel.activate(&entries);
        assert_eq!(panel.handle_key(KeyCode::Down, &entries), AgentPanelAction::Consumed);
        assert_eq!(panel.handle_key(KeyCode::Down, &entries), AgentPanelAction::Consumed);
        // 主(0) → one(1) → two(2)，Enter 应返回 two 的 cell_index
        assert_eq!(
            panel.handle_key(KeyCode::Enter, &entries),
            AgentPanelAction::Apply(Some(5))
        );
        assert!(!panel.is_active());
    }

    #[test]
    fn enter_on_main_returns_none() {
        let mut panel = AgentPanelState::default();
        let entries = vec![entry(2, "one", true)];
        panel.activate(&entries);
        assert_eq!(panel.handle_key(KeyCode::Enter, &entries), AgentPanelAction::Apply(None));
    }

    #[test]
    fn escape_and_other_keys_leave_panel() {
        let mut panel = AgentPanelState::default();
        let entries = vec![entry(2, "one", true)];
        panel.activate(&entries);
        assert_eq!(panel.handle_key(KeyCode::Esc, &entries), AgentPanelAction::Consumed);
        assert!(!panel.is_active());
        panel.activate(&entries);
        assert_eq!(
            panel.handle_key(KeyCode::Char('x'), &entries),
            AgentPanelAction::Ignored
        );
        assert!(!panel.is_active());
    }

    #[test]
    fn hint_line_appears_when_inactive() {
        let panel = AgentPanelState::default();
        let entries = vec![entry(2, "one", true)];
        let lines = panel.panel_lines(&entries);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains('↓'));
    }

    #[test]
    fn active_panel_lists_main_and_subagents() {
        let mut panel = AgentPanelState::default();
        let entries = vec![entry(2, "检查项目", true)];
        panel.activate(&entries);
        let lines = panel.panel_lines(&entries);
        assert!(lines.len() >= 3);
        assert!(lines.iter().any(|line| line.contains('❯')));
        assert!(lines.iter().any(|line| line.contains("检查项目")));
    }
}
