use crate::render::work_status::STATUS_PULSE_FRAMES;
use crate::render::ReasoningDisplayMode;

/// reasoning 内容的原始 source 数据。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReasoningCell {
    pub(crate) source: String,
}

/// 依据当前展示模式渲染 reasoning 内容。
///
/// 参数:
/// - `cell`: reasoning 源数据
/// - `mode`: reasoning 展示模式
///
/// 返回:
/// - ANSI 文本块
pub(crate) fn render(cell: &ReasoningCell, mode: ReasoningDisplayMode) -> String {
    match mode {
        ReasoningDisplayMode::Hidden => String::new(),
        ReasoningDisplayMode::Summary => {
            let lines = cell.source.lines().count().max(1);
            let chars = cell.source.chars().count();
            format!("\x1b[2m\x1b[36m• thinking {lines} lines · {chars} chars\x1b[0m")
        }
        ReasoningDisplayMode::Full => format!("\x1b[36m• thinking\n{}\x1b[0m", cell.source),
    }
}

/// 渲染流式阶段持续刷新的 reasoning 摘要。
///
/// 参数:
/// - `source`: 当前累计的 reasoning 原文
/// - `mode`: 当前 reasoning 展示模式
/// - `frame`: 跳动动画帧序号
///
/// 返回:
/// - 可直接显示的 ANSI 摘要行
pub(crate) fn render_live(source: &str, mode: ReasoningDisplayMode, frame: usize) -> String {
    if mode == ReasoningDisplayMode::Hidden || source.is_empty() {
        return String::new();
    }
    let pulse = STATUS_PULSE_FRAMES[frame % STATUS_PULSE_FRAMES.len()];
    let lines = source.lines().count().max(1);
    let chars = source.chars().count();
    format!("\x1b[2m\x1b[36m{pulse} thinking · {lines} lines · {chars} chars\x1b[0m")
}
