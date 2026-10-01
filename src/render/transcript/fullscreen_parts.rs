//! 全屏视图中命令卡片的分段：命令行与输出各自成为可点击展开的段落。
//!
//! 命令卡片在同一次渲染里输出 `$ command` 与输出两块；分段渲染时在两块之间
//! 插入私用区分界行，折行后据此切开并删除分界行，得到两段独立的行范围。

use super::cell::HistoryCell;
use super::line::AnsiLine;
use super::store::TranscriptRenderOptions;
use super::tool_cell::ToolCell;
use crate::render::omitted_line::{with_fullscreen_hints, EXPANDED_MARKER, FOLD_MARKER};
use crate::render::render_expand::{
    with_force_collapse, with_part_expansion, ExpandPart, PART_BOUNDARY,
};

/// 段落在 cell 中的位置：整块，或命令卡片的命令行、输出。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ParagraphPart {
    /// 整个 cell 作为一段（思考、diff、普通工具等）
    Whole,
    /// 命令卡片的一段
    Segment(ExpandPart),
}

/// 段落键：cell 下标与段落位置。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ParagraphKey {
    /// cell 下标，流式思考尾部为 `usize::MAX`
    pub(crate) cell: usize,
    pub(crate) part: ParagraphPart,
}

impl ParagraphKey {
    /// 【全屏视图】【段落键】构造整块段落键。
    /// @param cell 为 cell 下标
    /// @returns 段落键
    pub(crate) fn whole(cell: usize) -> Self {
        Self {
            cell,
            part: ParagraphPart::Whole,
        }
    }

    /// 【全屏视图】【段落键】构造命令卡片分段键。
    /// @param cell 为 cell 下标；part 为命令或输出
    /// @returns 段落键
    pub(crate) fn segment(cell: usize, part: ExpandPart) -> Self {
        Self {
            cell,
            part: ParagraphPart::Segment(part),
        }
    }
}

/// 一个 cell 渲染出的行与其中可点击段落的相对行范围。
pub(super) struct RenderedParts {
    pub(super) lines: Vec<AnsiLine>,
    /// (段落键, 相对起始行, 相对结束行, 是否展开)
    pub(super) spans: Vec<(ParagraphKey, usize, usize, bool)>,
}

/// 【全屏视图】【分段判断】判断 cell 是否按命令行与输出分段。
///
/// 只有本地 `!` 命令与前台 `run_command` 卡片同时含命令行和输出块；
/// 后台命令、提升为后台的命令走专用视图，不分段。
///
/// @param cell 为历史单元
/// @returns 需要分段时为 true
pub(super) fn is_segmented(cell: &HistoryCell) -> bool {
    match cell {
        HistoryCell::Shell(_) => true,
        HistoryCell::Tool(ToolCell::Invocation(view)) => {
            view.name == "run_command" && view.has_command_output()
        }
        _ => false,
    }
}

/// 【全屏视图】【分段渲染】按命令行与输出各自的展开状态渲染命令卡片。
///
/// 只把实际被折叠过的一段登记为可点击段落：命令很短时只有输出可点，
/// 输出很短时只有命令可点，避免点击后没有任何变化。
///
/// @param cell 为命令卡片；cell_index 为下标；width 为正文宽度；options 为渲染选项；
///        frame 为动画帧；is_open 判断某段是否已展开
/// @returns 渲染行与段落范围；没有分界行时返回 None，调用方按整块处理
pub(super) fn render_segmented(
    cell: &HistoryCell,
    cell_index: usize,
    width: usize,
    options: &TranscriptRenderOptions,
    frame: usize,
    is_open: impl Fn(ParagraphKey) -> bool,
) -> Option<RenderedParts> {
    let command_key = ParagraphKey::segment(cell_index, ExpandPart::Command);
    let output_key = ParagraphKey::segment(cell_index, ExpandPart::Output);
    let command_open = is_open(command_key);
    let output_open = is_open(output_key);
    // 1. 段外折叠块保持折叠，命令与输出按各自状态渲染
    let mut lines = with_fullscreen_hints(|| {
        with_force_collapse(|| {
            with_part_expansion(command_open, output_open, || {
                cell.display_lines_framed(width, options, frame)
            })
        })
    });
    // 2. 找到分界行并删除，切出两段行范围
    let boundary = lines
        .iter()
        .position(|line| line.as_str().contains(PART_BOUNDARY))?;
    lines.remove(boundary);
    let mut spans = Vec::new();
    if command_open || has_fold_hint(&lines[..boundary]) {
        spans.push((command_key, 0, boundary, command_open));
    }
    if boundary < lines.len() && (output_open || has_fold_hint(&lines[boundary..])) {
        spans.push((output_key, boundary, lines.len(), output_open));
    }
    Some(RenderedParts { lines, spans })
}

/// 【全屏视图】【折叠判断】判断一段行中是否含有折叠提示。
/// @param lines 为渲染行
/// @returns 含折叠或展开标记时为 true
fn has_fold_hint(lines: &[AnsiLine]) -> bool {
    lines.iter().any(|line| {
        let text = line.as_str();
        text.contains(FOLD_MARKER) || text.contains(EXPANDED_MARKER)
    })
}
