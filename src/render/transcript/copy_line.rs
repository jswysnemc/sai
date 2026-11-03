//! 显示行的复制范围与续行来源；内部标记在生成终端行时移除。

use std::borrow::Cow;

const MARKER: &str = "\x1b]777;sai-copy=";

/// 【终端】【复制来源】区分原始换行、排版续行和排版补入的空白。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct CopyLine {
    pub(crate) join_previous: bool,
    pub(crate) content_start: usize,
    pub(crate) content_end: Option<usize>,
    pub(crate) space_before: bool,
}

impl CopyLine {
    /// 【终端】【复制缩进】增加视觉前缀时平移正文范围，不改变换行来源。
    /// 参数: `indent` 为增加的显示列数；返回: 平移后的复制范围。
    pub(super) fn shifted(mut self, indent: usize) -> Self {
        self.content_start = self.content_start.saturating_add(indent);
        self.content_end = self.content_end.map(|end| end.saturating_add(indent));
        self
    }
}

/// 【终端】【续行传递】为预先折行的文本保存来源，后续 ANSI 样式处理不改变标记。
/// 参数: `text` 为已排版行，`join_previous` 表示是否继续上一原始行；返回: 内部渲染文本。
pub(crate) fn tag(text: &str, join_previous: bool) -> String {
    if text.is_empty() || !crate::render::tui_mode::prefers_fullscreen() {
        return text.to_string();
    }
    format!("{MARKER}{}\x07{text}", usize::from(join_previous))
}

/// 【终端】【续行解析】移除内部标记，记录标记前已有的视觉缩进。
/// 参数: `text` 为中间渲染行；返回: 无内部标记的文本与复制范围。
pub(crate) fn extract(text: &str) -> (Cow<'_, str>, CopyLine) {
    let Some(start) = text.find(MARKER) else {
        return (Cow::Borrowed(text), CopyLine::default());
    };
    let flag = start + MARKER.len();
    if !matches!(text.as_bytes().get(flag), Some(b'0' | b'1'))
        || text.as_bytes().get(flag + 1) != Some(&b'\x07')
    {
        return (Cow::Borrowed(text), CopyLine::default());
    }
    let copy = CopyLine {
        join_previous: text.as_bytes()[flag] == b'1',
        content_start: crate::render::ansi_clip::ansi_visible_width(&text[..start]),
        ..CopyLine::default()
    };
    (
        Cow::Owned(format!("{}{}", &text[..start], &text[flag + 2..])),
        copy,
    )
}
