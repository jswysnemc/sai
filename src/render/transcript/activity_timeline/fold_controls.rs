//! 输出正文内部折叠提示的显式点击范围。

use super::super::fullscreen_parts::ParagraphKey;
use super::super::fullscreen_view::{FullscreenDocument, ParagraphSpan};

/// 【终端】【隐藏提示控件】给正文中的 hidden 提示登记展开入口，沿用节点键和标题锚点。
/// 参数: `document` 为分组文档，`key` 为节点键，`start`、`end` 为节点范围；返回: 无。
pub(super) fn append(
    document: &mut FullscreenDocument,
    key: ParagraphKey,
    start: usize,
    end: usize,
) {
    let marker = format!(
        "\x1b[2m\x1b[36m{}",
        crate::render::omitted_line::FOLD_MARKER
    );
    for row in start + 1..end {
        let line = document.lines[row].as_str();
        if line.contains(&marker)
            && crate::render::ansi_clip::strip_ansi(line)
                .trim_start_matches([' ', '│', '┃'])
                .starts_with(crate::render::omitted_line::FOLD_MARKER)
        {
            document.paragraphs.push(ParagraphSpan {
                key,
                start,
                end,
                expanded: false,
                control: row,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::transcript::AnsiLine;

    /// 【终端】【窄提示回归】提示折行后仍登记符号所在行，不依赖完整 hidden 单词是否可见。
    /// 参数: 无；返回: 无。
    #[test]
    fn wrapped_hint_keeps_its_expand_control() {
        let mut document = FullscreenDocument {
            lines: vec![AnsiLine::new("title".into())],
            ..Default::default()
        };
        document.lines.extend(AnsiLine::wrap_block(
            &crate::render::omitted_line::render_omitted_line(30, true),
            12,
        ));
        let end = document.lines.len();
        append(&mut document, ParagraphKey::whole(0), 0, end);
        assert_eq!(document.paragraphs.len(), 1);
        assert_eq!(document.paragraphs[0].control, 1);
    }

    /// 【终端】【引导提示回归】统一引导列不能遮蔽正文隐藏提示的点击范围；无参数，无返回值
    #[test]
    fn guided_hint_keeps_its_expand_control() {
        let hint = crate::render::omitted_line::render_omitted_line(30, true);
        let mut document = FullscreenDocument {
            lines: vec![AnsiLine::new("title".into()), AnsiLine::new(hint)],
            ..Default::default()
        };
        super::super::super::tool_guides::decorate_node(&mut document.lines);
        append(&mut document, ParagraphKey::whole(0), 0, 2);
        assert_eq!(document.paragraphs.len(), 1);
        assert_eq!(document.paragraphs[0].control, 1);
    }

    /// 【终端】【正文识别回归】输出中普通 hidden 文字和输入标题不冒充折叠控件。
    /// 参数: 无；返回: 无。
    #[test]
    fn output_text_does_not_create_fake_fold_controls() {
        let mut document = FullscreenDocument {
            lines: vec![
                AnsiLine::new("title".into()),
                AnsiLine::new("▸ 30 lines hidden".into()),
                AnsiLine::new("\x1b[2m▸ Input:\x1b[0m".into()),
            ],
            ..Default::default()
        };
        append(&mut document, ParagraphKey::whole(0), 0, 3);
        assert!(document.paragraphs.is_empty());
    }
}
