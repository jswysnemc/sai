use super::diff_cell::{self, DiffCell};
use super::line::AnsiLine;
use super::markdown_cell::{self, MarkdownCell};
use super::meta_cell::{self, MetaCell};
use super::reasoning_cell::{self, ReasoningCell};
use super::shell_cell::{self, ShellCell};
use super::tool_cell::{self, ToolCell};
use super::user_echo_cell::{self, UserEchoCell};
use super::welcome_cell::{self, WelcomeCell};
use super::TranscriptRenderOptions;

/// REPL 用户输入的模式。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TranscriptMode {
    Plan,
    Yolo,
}

/// REPL 历史的 source-backed cell。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HistoryCell {
    UserEcho(UserEchoCell),
    Markdown(MarkdownCell),
    Reasoning(ReasoningCell),
    Shell(ShellCell),
    Tool(ToolCell),
    Diff(DiffCell),
    Meta(MetaCell),
    Welcome(WelcomeCell),
}

impl HistoryCell {
    /// 依据当前终端宽度预渲染 cell。
    ///
    /// 参数:
    /// - `width`: 当前终端列数
    /// - `options`: transcript 渲染选项
    ///
    /// 返回:
    /// - 按终端宽度预换行的 ANSI 行
    pub(crate) fn display_lines(
        &self,
        width: usize,
        options: &TranscriptRenderOptions,
    ) -> Vec<AnsiLine> {
        if let Self::Welcome(cell) = self {
            return welcome_cell::display_lines(cell, width);
        }
        let rendered = match self {
            Self::UserEcho(cell) => user_echo_cell::render(cell),
            Self::Markdown(cell) => markdown_cell::render(cell),
            Self::Reasoning(cell) => reasoning_cell::render(cell, options.reasoning_mode),
            Self::Shell(cell) => shell_cell::render(cell),
            Self::Tool(cell) => tool_cell::render(cell, options.tool_call_mode),
            Self::Diff(cell) => diff_cell::render(cell),
            Self::Meta(cell) => meta_cell::render(cell),
            Self::Welcome(_) => unreachable!("welcome cell is handled before plain rendering"),
        };
        if rendered.is_empty() {
            Vec::new()
        } else {
            AnsiLine::wrap_block(&rendered, width)
        }
    }

    /// 构造用户输入回显 cell。
    ///
    /// 参数:
    /// - `mode`: 用户提交时的 REPL 模式
    /// - `text`: 原始输入文本
    ///
    /// 返回:
    /// - 用户输入回显 cell
    pub(crate) fn user_echo(mode: TranscriptMode, text: String) -> Self {
        Self::UserEcho(UserEchoCell { mode, text })
    }

    /// 构造助手 Markdown cell。
    ///
    /// 参数:
    /// - `source`: 原始 Markdown 文本
    ///
    /// 返回:
    /// - 助手 Markdown cell
    pub(crate) fn markdown(source: String) -> Self {
        Self::Markdown(MarkdownCell { source })
    }

    /// 构造 reasoning cell。
    ///
    /// 参数:
    /// - `source`: 原始 reasoning 文本
    ///
    /// 返回:
    /// - reasoning cell
    pub(crate) fn reasoning(source: String) -> Self {
        Self::Reasoning(ReasoningCell { source })
    }

    /// 构造本地 Shell 命令 cell。
    ///
    /// 参数:
    /// - `command`: Shell 命令
    /// - `output`: 合并后输出
    /// - `exit_code`: 可选退出码
    ///
    /// 返回:
    /// - Shell 历史 cell
    pub(crate) fn shell(command: String, output: String, exit_code: Option<i32>) -> Self {
        Self::Shell(ShellCell {
            command,
            output,
            exit_code,
        })
    }

    /// 构造 edit_file diff cell。
    ///
    /// 参数:
    /// - `arguments`: edit_file 原始参数
    ///
    /// 返回:
    /// - diff cell
    pub(crate) fn diff(arguments: String) -> Self {
        Self::Diff(DiffCell::from_arguments(arguments))
    }

    /// 构造元信息 cell。
    ///
    /// 参数:
    /// - `text`: 系统或控制命令文本
    ///
    /// 返回:
    /// - 元信息 cell
    pub(crate) fn meta(text: String) -> Self {
        Self::Meta(MetaCell { text })
    }

    /// 构造 REPL 启动信息 cell。
    ///
    /// 参数:
    /// - `cell`: 启动信息 source
    ///
    /// 返回:
    /// - 启动信息 history cell
    pub(crate) fn welcome(cell: WelcomeCell) -> Self {
        Self::Welcome(cell)
    }
}
