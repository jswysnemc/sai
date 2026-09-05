use std::ops::Range;

/// 输入框登记的原子块类型，用于输入与提交回显的统一样式。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InputAtomKind {
    Text,
    Image,
    File,
    Skill,
}

impl InputAtomKind {
    /// 返回原子块标签的终端样式。
    ///
    /// 参数: 无
    /// 返回: 区分附件类型的前景色与背景色序列
    pub(crate) fn style(self) -> &'static str {
        match self {
            Self::Text => "\x1b[48;5;25m\x1b[38;5;159m",
            Self::Image => "\x1b[48;5;89m\x1b[38;5;225m",
            Self::File => "\x1b[48;5;23m\x1b[38;5;158m",
            Self::Skill => "\x1b[48;5;58m\x1b[38;5;229m",
        }
    }
}

/// 提交回显中的真实原子块，范围以完整正文的 UTF-8 字节计量。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InputAtom {
    pub(crate) range: Range<usize>,
    pub(crate) label: String,
    pub(crate) kind: InputAtomKind,
}

/// 提交后的完整正文与原子块来源；普通方括号文本不携带原子块元数据。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct InputEcho {
    pub(crate) text: String,
    pub(crate) atoms: Vec<InputAtom>,
}

/// 渲染已登记的原子块，保留其间的普通正文。
///
/// 参数: `text` 为完整正文，`atoms` 为按位置排序的原子块，`expanded` 控制长文本展开
/// 返回: 标签带附件样式的 ANSI 正文；无效范围按普通正文处理
pub(crate) fn render_input_atoms(text: &str, atoms: &[InputAtom], expanded: bool) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    for atom in atoms {
        if atom.range.start < cursor {
            continue;
        }
        let Some(before) = text.get(cursor..atom.range.start) else {
            continue;
        };
        let Some(body) = text.get(atom.range.clone()) else {
            continue;
        };
        // 1. 只给已登记的区间加标签样式，普通文本保持原样
        output.push_str(before);
        output.push_str(atom.kind.style());
        output.push_str(&atom.label);
        output.push_str("\x1b[0m");
        // 2. 全文视图保留标签并展示长文本原文
        if expanded && atom.kind == InputAtomKind::Text {
            output.push('\n');
            output.push_str(body);
        }
        cursor = atom.range.end;
    }
    output.push_str(&text[cursor..]);
    output
}
