use super::visible_width;

/// Markdown 表格列的对齐方式。
#[derive(Clone, Copy)]
pub(crate) enum TableAlign {
    Left,
    Center,
    Right,
}

/// 已渲染的表格单元格内容，携带终端显示宽度与图片元数据。
#[derive(Clone)]
pub(crate) struct CellContent {
    pub(crate) lines: Vec<String>,
    pub(crate) width: usize,
    pub(crate) is_image: bool,
    pub(crate) math_source: Option<String>,
}

impl CellContent {
    /// 从单行 ANSI 文本构造单元格。
    ///
    /// 参数:
    /// - `text`: 已完成样式渲染的单行文本
    ///
    /// 返回:
    /// - 文本单元格
    pub(crate) fn from_inline(text: String) -> Self {
        let width = visible_width(&text);
        Self {
            lines: vec![text],
            width,
            is_image: false,
            math_source: None,
        }
    }

    /// 构造空单元格。
    ///
    /// 返回:
    /// - 宽度为零的文本单元格
    pub(crate) fn empty() -> Self {
        Self {
            lines: vec![String::new()],
            width: 0,
            is_image: false,
            math_source: None,
        }
    }

    /// 构造终端图片单元格。
    ///
    /// 参数:
    /// - `lines`: 图片协议或半块文本行
    /// - `width`: 声明的终端列宽
    /// - `math_source`: 可选公式源码，用于按最终列宽重新渲染
    ///
    /// 返回:
    /// - 图片单元格
    pub(crate) fn from_image(
        lines: Vec<String>,
        width: usize,
        math_source: Option<String>,
    ) -> Self {
        Self {
            lines,
            width: width.max(1),
            is_image: true,
            math_source,
        }
    }
}
