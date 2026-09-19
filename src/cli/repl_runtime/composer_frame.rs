mod layout;
mod paint;
#[cfg(test)]
mod regression_tests;
mod styled_line;
#[cfg(test)]
mod tests;

use super::mention_panel::MentionPanel;
use super::shell_hint_panel::{bang_ghost_suffix, ShellHintPanel};
use super::slash_panel::SlashPanel;
use super::viewport::InlineViewport;
use crate::cli::repl_chrome::{
    chrome_input_content_cols, chrome_input_pad_row, chrome_input_row, ChromeInputPrefix,
    ReplChrome, CHROME_INPUT_INNER_PAD_ROWS, CHROME_INPUT_PAD_ROWS, CHROME_INPUT_PREFIX_COLS,
};
use crate::cli::repl_clipboard::ReplClipboardBlockSpan;
use crate::cli::repl_input_render::{
    repl_cursor_position_for_cols, repl_prompt_rows_for_cols, repl_visible_input_lines,
};
use crate::cli::repl_mentions::MentionSuggestion;
use crate::cli::repl_text::repl_input_lines;
use crate::cli::REPL_MAX_VISIBLE_INPUT_ROWS;
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::queue;
use crossterm::terminal::{Clear, ClearType};
use std::io::Write;

/// composer 一次绘制的完整内容签名。
///
/// 覆盖屏幕上会出现的每一处：位置、各区域文本与光标落点。
/// 两次签名相同即代表重绘不会改变任何像素。
#[derive(Clone, PartialEq, Eq)]
pub(super) struct ComposerSignature {
    top: u16,
    height: u16,
    cols: usize,
    lines: Vec<String>,
    cursor_col: u16,
    cursor_row: u16,
}

/// 可从输入 source 按当前终端宽度重绘的 REPL composer。
#[derive(Clone)]
pub(super) struct ComposerFrame {
    chrome: ReplChrome,
    input: String,
    cursor: usize,
    is_pasted: bool,
    clipboard_blocks: Vec<ReplClipboardBlockSpan>,
    slash_selection: usize,
    /// 与输入快照一致的引用候选，绘制期间不访问文件系统
    mention_candidates: Vec<MentionSuggestion>,
    /// 输入框上方的沉底面板行（todo 快照 / 排队消息 / agent 提示）
    panel_lines: Vec<String>,
    /// 是否已用 Esc 收起补全面板（slash / @ / #）
    ///
    /// 面板完全由输入内容推导，没有独立开关；Esc 收起后必须记住状态，
    /// 否则下一次重绘会立刻把面板再画回来
    panels_dismissed: bool,
    /// 模型是否正在运行；为 true 时斜杠面板把打断类命令置灰
    streaming: bool,
}

impl ComposerFrame {
    /// 创建当前输入状态的 composer source。
    ///
    /// 参数:
    /// - `chrome`: 底栏状态
    /// - `input`: 原始输入文本
    /// - `cursor`: 光标字符偏移
    /// - `is_pasted`: 是否为粘贴内容
    /// - `clipboard_blocks`: 剪贴板原子块区间
    /// - `slash_selection`: slash 命令面板的当前选中项
    ///
    /// 返回:
    /// - 可重绘的 composer source
    pub(super) fn new(
        chrome: ReplChrome,
        input: String,
        cursor: usize,
        is_pasted: bool,
        clipboard_blocks: Vec<ReplClipboardBlockSpan>,
        slash_selection: usize,
    ) -> Self {
        Self {
            chrome,
            input,
            cursor,
            is_pasted,
            clipboard_blocks,
            slash_selection,
            mention_candidates: Vec::new(),
            panel_lines: Vec::new(),
            panels_dismissed: false,
            streaming: false,
        }
    }

    /// 设置模型是否正在运行。
    ///
    /// 参数:
    /// - `streaming`: 运行中为 true，斜杠面板据此置灰打断类命令
    ///
    /// 返回:
    /// - 无
    pub(super) fn set_streaming(&mut self, streaming: bool) {
        self.streaming = streaming;
    }

    /// 设置是否已用 Esc 收起补全面板。
    ///
    /// 参数:
    /// - `dismissed`: 收起为 true
    ///
    /// 返回:
    /// - 无
    pub(super) fn set_panels_dismissed(&mut self, dismissed: bool) {
        self.panels_dismissed = dismissed;
    }

    /// 设置已经完成并验证的引用候选。
    ///
    /// 参数:
    /// - `items`: 当前输入对应的候选
    ///
    /// 返回:
    /// - 无
    pub(super) fn set_mention_candidates(&mut self, items: Vec<MentionSuggestion>) {
        self.mention_candidates = items;
    }

    /// 设置输入框上方的沉底面板行。
    ///
    /// 参数:
    /// - `lines`: 已按当前宽度截断的 ANSI 面板行
    ///
    /// 返回:
    /// - 无
    pub(super) fn set_panel_lines(&mut self, lines: Vec<String>) {
        self.panel_lines = lines;
    }

    /// 返回当前固化在 frame 中的沉底面板行。
    ///
    /// 返回:
    /// - 面板 ANSI 行
    pub(super) fn panel_lines(&self) -> &[String] {
        &self.panel_lines
    }

    /// 返回当前 composer 绑定的 chrome 状态。
    ///
    /// 返回:
    /// - chrome 引用
    pub(super) fn chrome(&self) -> &ReplChrome {
        &self.chrome
    }

    /// 返回 composer 在指定终端宽度下的视觉行数。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    ///
    /// 返回:
    /// - composer 所需视觉行数
    pub(super) fn height(&self, cols: usize) -> u16 {
        let layout = self.layout(cols);
        let panel_rows = self.panel_lines.len().min(usize::from(u16::MAX)) as u16;
        // 输入条自身厚度：内部上下背景边距 + 输入行
        let input_block = CHROME_INPUT_INNER_PAD_ROWS
            .saturating_mul(2)
            .saturating_add(layout.input_rows);
        // slash / mention / shell 提示时收起状态行，输入上方保留一行空白
        if layout.mention_panel.is_visible() {
            return panel_rows
                .saturating_add(CHROME_INPUT_PAD_ROWS)
                .saturating_add(input_block)
                .saturating_add(layout.mention_panel.height());
        }
        if layout.slash_panel.is_visible() {
            return panel_rows
                .saturating_add(CHROME_INPUT_PAD_ROWS)
                .saturating_add(input_block)
                .saturating_add(layout.slash_panel.height());
        }
        if layout.shell_hint.is_visible() {
            return panel_rows
                .saturating_add(CHROME_INPUT_PAD_ROWS)
                .saturating_add(input_block)
                .saturating_add(layout.shell_hint.height());
        }
        panel_rows
            .saturating_add(CHROME_INPUT_PAD_ROWS)
            .saturating_add(input_block)
            .saturating_add(1)
    }
}

/// composer 在单一终端宽度下的计算结果。
struct ComposerLayout {
    styled_display_lines: Vec<String>,
    input_rows: u16,
    slash_panel: SlashPanel,
    mention_panel: MentionPanel,
    shell_hint: ShellHintPanel,
    cursor_col: u16,
    cursor_row_offset: u16,
}
