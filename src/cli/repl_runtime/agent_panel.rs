mod render;
#[cfg(test)]
mod tests;

use crate::i18n::text as t;
use crate::render::transcript::SubagentOverviewEntry;
use crossterm::event::KeyCode;
use render::*;

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
    /// 离开面板，焦点回到输入框并收成单行
    Exit,
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
                if self.selected == 0 {
                    // 主项再按 ↑：收成单行，焦点回到输入框
                    self.deactivate();
                    AgentPanelAction::Exit
                } else {
                    self.selected -= 1;
                    AgentPanelAction::Consumed
                }
            }
            KeyCode::Down => {
                if self.selected + 1 < total {
                    self.selected += 1;
                }
                AgentPanelAction::Consumed
            }
            KeyCode::Enter => {
                let choice = if self.selected == 0 {
                    None
                } else {
                    entries.get(self.selected - 1).map(|entry| entry.cell_index)
                };
                self.deactivate();
                AgentPanelAction::Apply(choice)
            }
            KeyCode::Esc => {
                self.deactivate();
                AgentPanelAction::Exit
            }
            // 其他键：退出面板并交回常规输入处理
            _ => {
                self.deactivate();
                AgentPanelAction::Ignored
            }
        }
    }

    /// 渲染面板行（未截断，由沉底面板统一截断到终端宽度）。
    ///
    /// 参数:
    /// - `entries`: 当前子 agent 概览
    /// - `frame`: live 动效帧序号，驱动运行中条目的流光点
    ///
    /// 返回:
    /// - 面板 ANSI 行；无子 agent 时为空
    pub(super) fn panel_lines(
        &self,
        entries: &[SubagentOverviewEntry],
        frame: usize,
    ) -> Vec<String> {
        if entries.is_empty() {
            return Vec::new();
        }
        if self.active {
            self.active_lines(entries, frame)
        } else {
            vec![idle_line(entries, frame)]
        }
    }

    /// 焦点态：引导点标题 + 主智能体 + 全部子智能体。
    fn active_lines(&self, entries: &[SubagentOverviewEntry], frame: usize) -> Vec<String> {
        let col_width = status_column_width(entries);
        let mut lines = vec![format!(
            "{} \x1b[2m{} · ↑↓ {} · Enter {} · ↑ {}\x1b[0m",
            guide_dot(entries, frame),
            t("subagents", "子任务"),
            t("select", "选择"),
            t("view", "查看"),
            t("input", "回输入框")
        )];
        lines.push(selection_line(
            self.selected == 0,
            &pad_left("", col_width),
            &format!(
                "{} {}",
                t("main agent", "主智能体"),
                t("(overview)", "(总览)")
            ),
        ));
        // 1. 只展示选中项附近的窗口，避免子任务数量挤占输入区
        let limit = 4usize;
        let start = self
            .selected
            .saturating_sub(2)
            .min(entries.len().saturating_sub(limit));
        for (position, entry) in entries.iter().enumerate().skip(start).take(limit) {
            let viewing_suffix = if entry.viewing {
                format!(" \x1b[36m({})\x1b[0m", t("viewing", "查看中"))
            } else {
                String::new()
            };
            lines.push(selection_line(
                self.selected == position + 1,
                &render_entry_left(entry, col_width, frame),
                &format!("{}{viewing_suffix}", render_entry_title(entry)),
            ));
            if self.selected == position + 1 {
                if let Some(detail) = entry
                    .detail
                    .as_deref()
                    .filter(|text| !text.trim().is_empty())
                {
                    lines.push(format!("    \x1b[2m{}\x1b[0m", clip_chars(detail, 64)));
                }
            }
        }
        if entries.len() > limit {
            lines.push(format!(
                "    \x1b[2m{}–{} / {} · ↑↓ {}\x1b[0m",
                start + 1,
                (start + limit).min(entries.len()),
                entries.len(),
                t("browse", "浏览")
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

    /// 返回当前正在查看的子智能体 ID。
    ///
    /// 供 `/msg` 留言命令把未显式指定目标的消息投递给查看中的子智能体。
    ///
    /// 返回:
    /// - 处于子智能体视图时返回其 ID
    pub(in crate::cli) fn viewing_subagent_id(&self) -> Option<String> {
        self.transcript.viewing_subagent_id().map(str::to_string)
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
                AgentPanelAction::Consumed | AgentPanelAction::Exit => Ok(true),
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
            self.queue_panel.deactivate();
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
    pub(in crate::cli) fn handle_agent_panel_key(&mut self, code: KeyCode) -> anyhow::Result<bool> {
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
