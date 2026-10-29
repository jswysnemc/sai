mod input_geometry;
mod input_hint;
mod input_viewport;
mod key_hints;
mod layout;
mod paint;
#[cfg(test)]
mod regression_tests;
mod shortcut_sheet;
mod styled_line;
#[cfg(test)]
mod tests;

use super::mention_panel::MentionPanel;
use super::panel_mouse::PanelAction;
use super::shell_hint_panel::{bang_ghost_suffix, ShellHintPanel};
use super::slash_panel::SlashPanel;
use super::viewport::InlineViewport;
use crate::cli::repl_chrome::{
    chrome_input_content_cols, chrome_input_pad_row, chrome_input_row, ChromeInputPrefix,
    ReplChrome, CHROME_INPUT_INNER_PAD_ROWS, CHROME_INPUT_PAD_ROWS, CHROME_INPUT_PREFIX_COLS,
};
use crate::cli::repl_clipboard::ReplClipboardBlockSpan;
use crate::cli::repl_input_render::repl_cursor_position_for_cols;
use crate::cli::repl_mentions::MentionSuggestion;
use crate::cli::repl_text::repl_input_lines;
use crate::cli::REPL_MAX_VISIBLE_INPUT_ROWS;
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::queue;
use crossterm::terminal::{Clear, ClearType};
pub(super) use input_geometry::InputGeometry;
use std::io::Write;

/// composer 一次绘制的完整内容签名。
///
/// 覆盖屏幕上会出现的每一处：位置、各区域文本与光标落点。
/// 两次签名相同即代表重绘不会改变任何像素。
#[derive(Clone, PartialEq, Eq)]
pub(super) struct ComposerSignature {
    pub(super) input_geometry: Option<InputGeometry>,
    pub(super) footer_geometry: Option<InputGeometry>,
    top: u16,
    height: u16,
    cols: usize,
    lines: Vec<String>,
    cursor_col: u16,
    cursor_row: u16,
    /// 原始面板行宽与裁掉的顶部行数，供鼠标命中映射
    panel_widths: Vec<usize>,
    panel_actions: Vec<(usize, PanelAction)>,
    hidden_rows: usize,
}

impl ComposerSignature {
    /// 【终端】【操作快照】按最后一帧的可见行查询操作，不在鼠标移动时重新扫描会话。
    /// 参数: `column` 与 `row` 为屏幕坐标；返回: 与显示内容对应的操作。
    pub(super) fn panel_action_at(&self, column: u16, row: u16) -> Option<PanelAction> {
        let index = self.panel_row_at(column, row)?;
        self.panel_actions
            .iter()
            .find(|(candidate, _)| *candidate == index)
            .map(|(_, action)| *action)
    }
    /// 【终端】【面板测试】参数为源面板行号；返回实际绘制行，仅供断言。
    #[cfg(test)]
    pub(super) fn panel_line(&self, row: usize) -> Option<&str> {
        self.lines
            .get(row.checked_sub(self.hidden_rows)?)
            .map(String::as_str)
    }
    /// 【终端】【面板命中】按实际裁切行和显示宽度定位，空白及不可见行不命中。
    /// 参数: `column` 与 `row` 为屏幕坐标；返回: 原面板行号。
    pub(super) fn panel_row_at(&self, column: u16, row: u16) -> Option<usize> {
        let relative = row.checked_sub(self.top)?;
        if relative >= self.height {
            return None;
        }
        let index = usize::from(relative) + self.hidden_rows;
        self.panel_widths
            .get(index)
            .filter(|width| usize::from(column) < **width)
            .map(|_| index)
    }

    /// 【全屏视图】【面板点击】参数为终端列与行，返回是否命中实际可见的首个面板标题。
    #[cfg(test)]
    pub(in crate::cli::repl_runtime) fn hits_panel_header(&self, column: u16, row: u16) -> bool {
        self.panel_row_at(column, row) == Some(0)
    }
}

/// 可从输入 source 按当前终端宽度重绘的 REPL composer。
#[derive(Clone)]
pub(super) struct ComposerFrame {
    selection: super::composer_selection::ComposerSelection,
    chrome: ReplChrome,
    input: String,
    cursor: usize,
    clipboard_blocks: Vec<ReplClipboardBlockSpan>,
    slash_selection: usize,
    /// 与输入快照一致的引用候选，绘制期间不访问文件系统
    mention_candidates: Vec<MentionSuggestion>,
    /// 输入框上方的沉底面板行（todo 快照 / 排队消息 / agent 提示）
    panel_lines: Vec<String>,
    panel_pointer: Option<(u16, u16)>,
    panel_actions: Vec<(usize, PanelAction)>,
    panel_expanded_rows: Vec<usize>,
    /// 是否已用 Esc 收起补全面板（slash / @ / #）
    ///
    /// 面板完全由输入内容推导，没有独立开关；Esc 收起后必须记住状态，
    /// 否则下一次重绘会立刻把面板再画回来
    panels_dismissed: bool,
    /// 模型是否正在运行；为 true 时斜杠面板把打断类命令置灰
    streaming: bool,
    /// 空输入时替代默认提示的占位文本（SSH 征询等临时输入场景）
    placeholder_override: Option<String>,
    /// 状态栏下方按键提示所需的界面状态
    key_hints: key_hints::KeyHintContext,
    /// 快捷键速查面板是否展开（只在空输入时显示）
    shortcuts_open: bool,
    /// 速查面板展示的图片粘贴键
    paste_key: crate::config::PasteImageKey,
}

impl ComposerFrame {
    /// 【终端】【输入选区】同步源码选区，绘制时再映射实际屏幕坐标。
    /// 参数: `selection` 为当前选区；返回: 无。
    pub(super) fn set_selection(
        &mut self,
        selection: super::composer_selection::ComposerSelection,
    ) {
        self.selection = selection;
    }
    /// 【终端】【面板反馈】保存指针和可操作行，绘制时使用实际几何计算悬停。
    /// 参数: `pointer` 为屏幕坐标，`actions` 为可点击面板行，`expanded_rows` 为已展开标题行；返回: 无。
    pub(super) fn set_panel_pointer(
        &mut self,
        pointer: Option<(u16, u16)>,
        actions: Vec<(usize, PanelAction)>,
        expanded_rows: Vec<usize>,
    ) {
        self.panel_pointer = pointer;
        self.panel_actions = actions;
        self.panel_expanded_rows = expanded_rows;
    }

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
        _is_pasted: bool,
        clipboard_blocks: Vec<ReplClipboardBlockSpan>,
        slash_selection: usize,
    ) -> Self {
        Self {
            selection: Default::default(),
            chrome,
            input,
            cursor,
            clipboard_blocks,
            slash_selection,
            mention_candidates: Vec::new(),
            panel_lines: Vec::new(),
            panel_pointer: None,
            panel_actions: Vec::new(),
            panel_expanded_rows: Vec::new(),
            panels_dismissed: false,
            streaming: false,
            placeholder_override: None,
            key_hints: key_hints::KeyHintContext::default(),
            shortcuts_open: false,
            paste_key: crate::config::PasteImageKey::default(),
        }
    }

    /// 设置空输入时显示的占位文本。
    ///
    /// 参数:
    /// - `placeholder`: 纯文本占位提示；None 恢复默认提示
    ///
    /// 返回:
    /// - 无
    pub(super) fn set_placeholder_override(&mut self, placeholder: Option<String>) {
        self.placeholder_override = placeholder;
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

    /// 设置按键提示行使用的界面状态。
    ///
    /// 参数:
    /// - `context`: 运行、全屏与退出确认状态
    ///
    /// 返回:
    /// - 无
    pub(super) fn set_key_hints(&mut self, context: key_hints::KeyHintContext) {
        self.key_hints = context;
    }

    /// 设置快捷键速查面板状态。
    ///
    /// 参数:
    /// - `open`: 是否展开
    /// - `paste_key`: 配置的图片粘贴键
    ///
    /// 返回:
    /// - 无
    pub(super) fn set_shortcuts(&mut self, open: bool, paste_key: crate::config::PasteImageKey) {
        self.shortcuts_open = open;
        self.paste_key = paste_key;
    }

    /// 返回输入框上方的全部行：沉底面板与快捷键速查。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    ///
    /// 返回:
    /// - 按显示顺序排列的行
    fn top_lines(&self, cols: usize) -> Vec<String> {
        let mut lines = self.panel_lines.clone();
        if self.shortcuts_open && self.input.is_empty() {
            // 面板可用行数：终端行数减去输入框其余部分，保留一行给上方的正文
            let rows = usize::from(super::viewport::TerminalSize::current().rows);
            let others = self.panel_lines.len()
                + usize::from(CHROME_INPUT_PAD_ROWS + CHROME_INPUT_INNER_PAD_ROWS * 2)
                + 1
                + 1
                + usize::from(KEY_HINT_ROWS);
            let max_rows = rows.saturating_sub(others + 1);
            lines.extend(shortcut_sheet::render_shortcut_sheet(
                cols,
                max_rows,
                self.paste_key,
            ));
        }
        lines
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
        let panel_rows = self.top_lines(cols).len().min(usize::from(u16::MAX)) as u16;
        // 输入条自身厚度：内部上下背景边距 + 输入行
        let input_block = CHROME_INPUT_INNER_PAD_ROWS
            .saturating_mul(2)
            .saturating_add(layout.input_rows);
        // slash / mention / shell 提示时收起状态行，输入上方保留一行空白
        if layout.mention_panel.is_visible() {
            return panel_rows
                .saturating_add(CHROME_INPUT_PAD_ROWS)
                .saturating_add(input_block)
                .saturating_add(layout.mention_panel.height())
                .saturating_add(KEY_HINT_ROWS);
        }
        if layout.slash_panel.is_visible() {
            return panel_rows
                .saturating_add(CHROME_INPUT_PAD_ROWS)
                .saturating_add(input_block)
                .saturating_add(layout.slash_panel.height())
                .saturating_add(KEY_HINT_ROWS);
        }
        if layout.shell_hint.is_visible() {
            return panel_rows
                .saturating_add(CHROME_INPUT_PAD_ROWS)
                .saturating_add(input_block)
                .saturating_add(layout.shell_hint.height())
                .saturating_add(KEY_HINT_ROWS);
        }
        panel_rows
            .saturating_add(CHROME_INPUT_PAD_ROWS)
            .saturating_add(input_block)
            .saturating_add(1)
            .saturating_add(KEY_HINT_ROWS)
    }
}

/// 状态栏下方按键提示占用的行数。
const KEY_HINT_ROWS: u16 = 1;

pub(super) use key_hints::{KeyHintContext, KeyNotice};

/// composer 在单一终端宽度下的计算结果。
struct ComposerLayout {
    first_visual_row: usize,
    styled_display_lines: Vec<String>,
    input_rows: u16,
    slash_panel: SlashPanel,
    mention_panel: MentionPanel,
    shell_hint: ShellHintPanel,
    cursor_col: u16,
    cursor_row_offset: u16,
}
