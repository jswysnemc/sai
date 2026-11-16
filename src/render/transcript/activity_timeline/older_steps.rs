use super::super::fullscreen_parts::ParagraphKey;
use super::super::fullscreen_view::{FullscreenDocument, ParagraphSpan};
use super::super::line::AnsiLine;
use std::collections::HashSet;

const RECENT_STEPS: usize = super::recent_details::DETAIL_WINDOW_ROWS;

/// 【终端】【旧步骤折叠】记录较早步骤的独立折叠状态，近期步骤始终按原顺序展示。
pub(super) struct OlderSteps {
    key: ParagraphKey,
    count: usize,
    open: bool,
}

impl OlderSteps {
    /// 【终端】【旧步骤折叠】保留十行窗口可能容纳的末尾十项，待审批组不隐藏任何步骤。
    /// 参数: `start` 为分组下标，`total` 为可见步骤数，`expanded` 为展开键，`permission` 为待审批状态；返回: 折叠策略。
    pub(super) fn new(
        start: usize,
        total: usize,
        expanded: &HashSet<ParagraphKey>,
        permission: bool,
    ) -> Self {
        let key = ParagraphKey::older_activity(start);
        Self {
            key,
            count: if permission {
                0
            } else {
                total.saturating_sub(RECENT_STEPS)
            },
            open: expanded.contains(&key),
        }
    }

    /// 【终端】【旧步骤阈值】返回默认收入较早步骤入口的分界序号。
    /// 参数: 无；返回: 应隐藏的前置步骤数量。
    pub(super) fn hidden_cutoff(&self) -> usize {
        self.count
    }

    /// 【终端】【旧步骤可见性】展开入口后恢复所有较早步骤，否则仅隐藏未受保护的较早步骤。
    /// 参数: `position` 为过滤隐藏偏好后的步骤序号，`pinned` 表示步骤是否正处于查看固定状态；返回: 是否应该渲染该步骤。
    pub(super) fn visible_with_pin(&self, position: usize, pinned: bool) -> bool {
        self.open || position >= self.count || pinned
    }

    /// 【终端】【旧步骤入口】保留折叠键，并在仍有实际隐藏步骤时添加独立可点击入口。
    /// 参数: `document` 为分组文档，`width` 为净宽，`group_open` 为整组状态，`inset` 为子层级缩进，`hidden_count` 为实际隐藏步骤数；返回: 无。
    pub(super) fn append_control(
        &self,
        document: &mut FullscreenDocument,
        width: usize,
        group_open: bool,
        inset: usize,
        hidden_count: usize,
    ) {
        document.retained_keys.push(self.key);
        if hidden_count == 0 || !group_open {
            return;
        }
        let noun = if hidden_count == 1 { "step" } else { "steps" };
        let label = format!(
            "{} {} {} earlier {noun}",
            if self.open { "▾" } else { "▸" },
            if self.open { "Hide" } else { "Show" },
            hidden_count
        );
        let row = document.lines.len();
        document.lines.push(AnsiLine::new(format!(
            "{}\x1b[2m{}\x1b[0m",
            " ".repeat(inset),
            crate::render::clip_to_width(&label, width, "…")
        )));
        document.paragraphs.push(ParagraphSpan {
            key: self.key,
            start: row,
            end: row + 1,
            expanded: self.open,
            control: row,
        });
    }
}
