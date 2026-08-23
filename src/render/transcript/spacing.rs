use super::cell::HistoryCell;
use super::line::AnsiLine;
use crate::render::activity_animation::strip_ansi_for_test;

/// 判断一行在终端上是否看起来是空行。
///
/// 参数:
/// - `line`: 预换行 ANSI 行
///
/// 返回:
/// - 去掉 ANSI 后没有非空白字符时为 true
pub(super) fn is_visual_blank(line: &AnsiLine) -> bool {
    strip_ansi_for_test(line.as_str()).trim().is_empty()
}

/// 去掉块首连续的视觉空行。
///
/// 参数:
/// - `lines`: 待修剪的行
///
/// 返回:
/// - 无
pub(super) fn trim_leading_visual_blanks(lines: &mut Vec<AnsiLine>) {
    while lines.first().is_some_and(is_visual_blank) {
        lines.remove(0);
    }
}

/// 去掉块尾连续的视觉空行，避免和下一块的前空行叠成两行。
///
/// 参数:
/// - `lines`: 待修剪的行
///
/// 返回:
/// - 无
pub(super) fn trim_trailing_visual_blanks(lines: &mut Vec<AnsiLine>) {
    while lines.last().is_some_and(is_visual_blank) {
        lines.pop();
    }
}

/// 若下一块以空行开头且上一行已经是空行，丢掉下一块的前导空行。
///
/// 参数:
/// - `next`: 即将接上的 live / 下一块
/// - `previous`: 已组装区的最后一行
///
/// 返回:
/// - 无
pub(super) fn drop_duplicate_leading_blank(next: &mut Vec<AnsiLine>, previous: Option<&AnsiLine>) {
    if previous.is_some_and(is_visual_blank) && next.first().is_some_and(is_visual_blank) {
        next.remove(0);
    }
}

/// 正文/提示之后的工具需要一块空行；思考或连续工具之间不加。
///
/// 参数:
/// - `previous`: 上一块 cell
/// - `next`: 下一块 cell
///
/// 返回:
/// - 两块之间应插入区块空行时为 true
pub(super) fn needs_section_gap(previous: &HistoryCell, next: &HistoryCell) -> bool {
    matches!(
        previous,
        HistoryCell::Markdown(_) | HistoryCell::Meta(_) | HistoryCell::UserEcho(_)
    ) && matches!(
        next,
        HistoryCell::Tool(_) | HistoryCell::Shell(_) | HistoryCell::Diff(_)
    )
}

/// 判断 live 区第一行有效内容是否为工具行。
///
/// 参数:
/// - `lines`: live 预换行
///
/// 返回:
/// - 首个非空行以 `•` 开头时为 true
pub(super) fn live_opens_with_tool(lines: &[AnsiLine]) -> bool {
    lines
        .iter()
        .find(|line| !is_visual_blank(line))
        .is_some_and(|line| {
            strip_ansi_for_test(line.as_str())
                .trim_start()
                .starts_with('•')
        })
}

/// 上一块是正文类时，给即将接上的工具 live 补一块前空行。
///
/// 参数:
/// - `live`: live 行
/// - `previous`: 定稿区最后一块
///
/// 返回:
/// - 无
pub(super) fn ensure_live_tool_gap(live: &mut Vec<AnsiLine>, previous: Option<&HistoryCell>) {
    let Some(previous) = previous else {
        return;
    };
    if !matches!(
        previous,
        HistoryCell::Markdown(_) | HistoryCell::Meta(_) | HistoryCell::UserEcho(_)
    ) {
        return;
    }
    if !live_opens_with_tool(live) {
        return;
    }
    if live.first().is_some_and(is_visual_blank) {
        return;
    }
    live.insert(0, AnsiLine::new(String::new()));
}

/// 若当前最后一行不是空行，补一块区块空行。
///
/// 参数:
/// - `lines`: 已组装的行
///
/// 返回:
/// - 无
pub(super) fn ensure_blank_before(lines: &mut Vec<AnsiLine>) {
    if lines.last().is_some_and(|line| !is_visual_blank(line)) {
        lines.push(AnsiLine::new(String::new()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【块间距】验证折行产生的 `\x1b[0m` 空行也被视为视觉空行。
    #[test]
    fn ansi_only_line_is_visual_blank() {
        assert!(is_visual_blank(&AnsiLine::new(String::new())));
        assert!(is_visual_blank(&AnsiLine::new("\x1b[0m".into())));
        assert!(is_visual_blank(&AnsiLine::new("  \x1b[0m".into())));
        assert!(!is_visual_blank(&AnsiLine::new("◦ Thinking".into())));
    }

    /// 【终端】【块间距】验证接缝处不会保留连续两行空白。
    #[test]
    fn duplicate_leading_blank_is_dropped() {
        let previous = AnsiLine::new("\x1b[0m".into());
        let mut next = vec![AnsiLine::new(String::new()), AnsiLine::new("  body".into())];
        drop_duplicate_leading_blank(&mut next, Some(&previous));
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].as_str(), "  body");
    }

    /// 【终端】【块间距】正文后的工具要空行，思考后的工具不要。
    #[test]
    fn section_gap_only_between_body_and_tool() {
        let body = HistoryCell::markdown("answer".into());
        let think = HistoryCell::reasoning("plan".into());
        let tool = HistoryCell::shell("ls".into(), String::new(), Some(0));
        assert!(needs_section_gap(&body, &tool));
        assert!(!needs_section_gap(&think, &tool));
        assert!(!needs_section_gap(&tool, &tool));
    }
}
