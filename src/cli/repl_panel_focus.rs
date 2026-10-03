use super::repl_commands::visible_repl_command_suggestions;
use super::repl_runtime::ReplRuntime;
use crossterm::event::{KeyCode, KeyModifiers};

/// 【终端】【面板焦点】按当前实际可见状态确定按键归属
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PanelFocus {
    Input,
    Completion,
    Queue,
    Agents,
}

impl PanelFocus {
    /// 返回分页键是否归面板所有，无参数
    pub(super) fn owns_viewport_keys(self) -> bool {
        self != Self::Input
    }
}

impl ReplRuntime {
    /// 返回当前焦点；input/cursor 为输入快照，已关闭的候选不持有焦点
    pub(in crate::cli) fn panel_focus(&self, input: &str, cursor: usize) -> PanelFocus {
        if self.queue_panel_active() {
            PanelFocus::Queue
        } else if self.agent_panel_active() {
            PanelFocus::Agents
        } else if !self.composer_panels_dismissed() && self.completion_count(input, cursor) > 0 {
            PanelFocus::Completion
        } else {
            PanelFocus::Input
        }
    }

    /// 处理候选面板导航；input/cursor 为草稿，selected 为选中项，code/modifiers 为按键
    /// 返回是否消费，Esc 只关闭候选，不清除输入或中断模型
    pub(in crate::cli) fn navigate_completion(
        &mut self,
        input: &str,
        cursor: usize,
        selected: &mut usize,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> bool {
        if !modifiers.is_empty()
            || self.queue_panel_active()
            || self.agent_panel_active()
            || self.composer_panels_dismissed()
        {
            return false;
        }
        let count = self.completion_count(input, cursor);
        // 【终端】【异步候选】查询可能刚以零结果结束，只使用本次取得的数量
        if count == 0 {
            return false;
        }
        match code {
            KeyCode::Esc => self.dismiss_composer_panels(input, cursor),
            KeyCode::Up => *selected = (*selected + count - 1) % count,
            KeyCode::Down => *selected = (*selected + 1) % count,
            KeyCode::PageUp => *selected = 0,
            KeyCode::PageDown => *selected = count - 1,
            _ => return false,
        }
        true
    }

    /// 统计可见候选，input/cursor 为草稿；返回引用优先的候选数
    fn completion_count(&self, input: &str, cursor: usize) -> usize {
        let mentions = self.mention_candidates(input, cursor).len();
        if self.mention_completion_pending() {
            return 1;
        }
        if mentions > 0 {
            mentions
        } else if cursor == input.chars().count() {
            visible_repl_command_suggestions(input, false).len()
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【技能补全】方向键遍历完整列表，首尾跳转后仍插入对应技能
    /// 参数: 无；返回: 无，候选不可达或插入错误时断言失败
    #[test]
    fn skill_completion_navigates_and_inserts_all_matches() {
        let mut runtime = ReplRuntime::new(
            100,
            crate::render::transcript::TranscriptRenderOptions {
                reasoning_mode: crate::render::ReasoningDisplayMode::Summary,
                tool_call_mode: crate::render::ToolCallDisplayMode::Summary,
            },
        );
        runtime.set_mention_skills(
            (0..12)
                .map(|index| (format!("skill-{index:02}"), String::new()))
                .collect(),
        );
        let trigger = crate::cli::repl_mentions::find_mention_trigger("#", 1).unwrap();
        let mut selected = 0;
        for (key, expected) in std::iter::repeat_n(KeyCode::Down, 12)
            .zip((1..12).chain(std::iter::once(0)))
            .chain([
                (KeyCode::Up, 11),
                (KeyCode::PageUp, 0),
                (KeyCode::PageDown, 11),
            ])
        {
            assert!(runtime.navigate_completion("#", 1, &mut selected, key, KeyModifiers::NONE));
            assert_eq!(selected, expected);
            let candidates = runtime.mention_candidates("#", 1);
            let (input, cursor) =
                crate::cli::repl_mentions::apply_mention("#", &trigger, &candidates[selected]);
            assert_eq!(input, format!("#skill-{expected:02} "));
            assert_eq!(cursor, input.chars().count());
        }
    }

    /// 候选分页和关闭不应进入历史面板，关闭后焦点恢复输入框
    #[test]
    fn completion_owns_navigation_until_dismissed() {
        let mut runtime = ReplRuntime::new(
            100,
            crate::render::transcript::TranscriptRenderOptions {
                reasoning_mode: crate::render::ReasoningDisplayMode::Summary,
                tool_call_mode: crate::render::ToolCallDisplayMode::Summary,
            },
        );
        let mut selected = 0;
        assert!(runtime.navigate_completion(
            "/context ",
            9,
            &mut selected,
            KeyCode::PageDown,
            KeyModifiers::NONE
        ));
        assert_eq!(selected, 1);
        assert!(runtime.navigate_completion(
            "/context ",
            9,
            &mut selected,
            KeyCode::Esc,
            KeyModifiers::NONE
        ));
        assert_eq!(runtime.panel_focus("/context ", 9), PanelFocus::Input);
        assert!(!runtime.navigate_completion(
            "/context ",
            9,
            &mut selected,
            KeyCode::Down,
            KeyModifiers::NONE
        ));
    }
}
