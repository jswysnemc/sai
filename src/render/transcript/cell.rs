use super::assistant_body;
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
    Automatic,
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
    /// - `width`: 视觉引导区右侧的正文净宽度
    /// - `options`: transcript 渲染选项
    ///
    /// 返回:
    /// - 按终端宽度预换行的 ANSI 行
    pub(crate) fn display_lines(
        &self,
        width: usize,
        options: &TranscriptRenderOptions,
    ) -> Vec<AnsiLine> {
        self.display_lines_framed(width, options, 0)
    }

    /// 带 live 动效帧渲染 cell；进行中的工具卡用扫光替换静态 `run` 徽标。
    pub(crate) fn display_lines_framed(
        &self,
        width: usize,
        options: &TranscriptRenderOptions,
        frame: usize,
    ) -> Vec<AnsiLine> {
        let lines = match self {
            Self::Welcome(cell) => welcome_cell::display_lines(cell, width),
            Self::Markdown(cell) => {
                // 【终端】【正文引导】正文渲染与折行共用调用方传入的净宽度
                let rendered = crate::render::render_width::with_render_width(width, || {
                    markdown_cell::render(cell)
                });
                assistant_body::display_lines(&rendered, width)
            }
            Self::Diff(cell) => display_diff_lines(cell, width, options.tool_call_mode),
            Self::UserEcho(cell) => display_rendered_lines(width, || user_echo_cell::render(cell)),
            Self::Reasoning(cell) => display_rendered_lines(width, || {
                reasoning_cell::render(cell, options.reasoning_mode)
            }),
            Self::Shell(cell) => display_rendered_lines(width, || shell_cell::render(cell)),
            Self::Tool(cell) => display_rendered_lines(width, || {
                tool_cell::render(cell, options.tool_call_mode, frame)
            }),
            Self::Meta(cell) => display_rendered_lines(width, || {
                // 历史烘焙的旧横线先剥掉，再按当前正文净宽为总览补一条 turn 分割线；
                // 分割线在渲染期生成，终端宽度变化时随重排自动重画
                let rendered = meta_cell::render(cell);
                let refit = crate::render::session_summary::refit_turn_rule(&rendered, width);
                if cell.kind == meta_cell::MetaKind::Summary {
                    // 总览正文与通栏分割线直接相连，扫读时作为同一块信息
                    format!("{refit}\n{}", turn_rule(width))
                } else {
                    refit
                }
            }),
        };
        // 区块间距（交界处只保留一行空行）：
        // - Reasoning：仅前空一行（后空行会与 Markdown/Meta 的前空行叠成两行）
        // - Markdown / Meta：前空一行（承接思考/工具与正文、总览）
        // - Tool / Shell：不加前空行，避免连续工具被撕开
        let spaced = if lines.is_empty() {
            lines
        } else if matches!(self, Self::Reasoning(_)) {
            let mut spaced = Vec::with_capacity(lines.len() + 1);
            spaced.push(AnsiLine::new(String::new()));
            spaced.extend(lines);
            spaced
        } else if matches!(self, Self::Markdown(_) | Self::Meta(_)) {
            let mut spaced = Vec::with_capacity(lines.len() + 1);
            spaced.push(AnsiLine::new(String::new()));
            spaced.extend(lines);
            spaced
        } else {
            lines
        };
        spaced
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
        Self::UserEcho(UserEchoCell::new(mode, text))
    }

    /// 构造可折叠的用户回显（粘贴长文本展开后）。
    ///
    /// 参数:
    /// - `mode`: 用户提交时的 REPL 模式
    /// - `text`: 已展开的回显正文
    /// - `fold`: 是否按思考块语义折叠
    ///
    /// 返回:
    /// - 用户输入回显 cell
    pub(crate) fn user_echo_with_fold(mode: TranscriptMode, text: String, fold: bool) -> Self {
        Self::UserEcho(UserEchoCell::with_fold(mode, text, fold))
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
    #[allow(dead_code)]
    pub(crate) fn reasoning(source: String) -> Self {
        Self::Reasoning(ReasoningCell::new(source))
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
    /// 构造编辑类 diff cell（写盘前冻结预览）。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `arguments`: 原始工具参数
    ///
    /// 返回:
    /// - diff cell
    pub(crate) fn diff(name: String, arguments: String) -> Self {
        Self::Diff(DiffCell::from_call(name, arguments))
    }

    /// 构造元信息 cell。
    ///
    /// 参数:
    /// - `text`: 系统或控制命令文本
    ///
    /// 返回:
    /// - 元信息 cell
    pub(crate) fn meta(text: String) -> Self {
        Self::Meta(MetaCell {
            text,
            kind: meta_cell::MetaKind::Notice,
        })
    }

    /// 构造轮次总览 cell，渲染时在其下追加 turn 分割线。
    ///
    /// 参数:
    /// - `text`: 自带样式的上下文总览文本
    ///
    /// 返回:
    /// - 轮次总览 cell
    pub(crate) fn turn_summary(text: String) -> Self {
        Self::Meta(MetaCell {
            text,
            kind: meta_cell::MetaKind::Summary,
        })
    }

    /// 构造失败提示 cell。
    ///
    /// 参数:
    /// - `text`: 轮次失败或中断说明
    ///
    /// 返回:
    /// - 失败提示 cell
    pub(crate) fn failure(text: String) -> Self {
        Self::Meta(MetaCell {
            text,
            kind: meta_cell::MetaKind::Failure,
        })
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

/// 【终端】【会话分隔】生成一条弱化的通栏 turn 分割线。
///
/// 参数:
/// - `width`: 正文净宽度
///
/// 返回:
/// - 恰好占满一行的弱化横线
fn turn_rule(width: usize) -> String {
    format!("\x1b[2m{}\x1b[0m", "─".repeat(width.max(1)))
}

/// 【终端】【会话渲染】按指定净宽度渲染并折行普通 transcript cell。
///
/// 参数:
/// - `width`: 调用方已经扣除视觉引导区后的正文列数
/// - `render`: 返回 ANSI 文本块的渲染函数
///
/// 返回:
/// - 按正文净宽度完成折行的终端行
fn display_rendered_lines<F>(width: usize, render: F) -> Vec<AnsiLine>
where
    F: FnOnce() -> String,
{
    let rendered = crate::render::render_width::with_render_width(width, render);
    if rendered.is_empty() {
        Vec::new()
    } else {
        AnsiLine::wrap_block(&rendered, width)
    }
}

/// 【终端】【会话渲染】按 diff 对称边距渲染并折行编辑内容。
///
/// 参数:
/// - `cell`: diff 源数据
/// - `width`: 调用方已经扣除视觉引导区后的正文列数
/// - `mode`: 工具展示模式（Hidden 不渲染；Summary/Full 含冻结正文）
///
/// 返回:
/// - 保留左右边距的 diff 终端行
fn display_diff_lines(
    cell: &DiffCell,
    width: usize,
    mode: crate::render::ToolCallDisplayMode,
) -> Vec<AnsiLine> {
    if mode == crate::render::ToolCallDisplayMode::Hidden {
        return Vec::new();
    }
    let content_width = width
        .saturating_sub(crate::render::content_indent::DIFF_BLOCK_INSET)
        .max(1);
    let rendered = crate::render::render_width::with_render_width(content_width, || {
        diff_cell::render(cell, mode)
    });
    if rendered.is_empty() {
        Vec::new()
    } else {
        // 续行缩进到 diff 正文列，长行折行后才不会顶到最左侧与行号列错位
        let body_column = crate::render::edit_diff::diff_body_start_column(&rendered);
        AnsiLine::wrap_block_with_right_margin_and_continuation_indent(
            &rendered,
            content_width,
            crate::render::content_indent::DIFF_BLOCK_INSET,
            body_column.max(crate::render::content_indent::DIFF_NESTED_INDENT),
        )
    }
}
