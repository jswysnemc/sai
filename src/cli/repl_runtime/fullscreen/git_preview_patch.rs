//! 将统一差异转换成带行号的界面数据，不直接显示 Git 控制头。

use super::sidebar_widgets::safe_text;

const MAX_LINES: usize = 8_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LineKind {
    Context,
    Added,
    Removed,
    Hunk,
    Notice,
}

pub(super) struct PatchLine {
    pub(super) old: Option<usize>,
    pub(super) new: Option<usize>,
    pub(super) kind: LineKind,
    pub(super) text: String,
}

#[derive(Default)]
pub(super) struct PreviewContent {
    pub(super) lines: Vec<PatchLine>,
    pub(super) code: Vec<String>,
    pub(super) added: usize,
    pub(super) removed: usize,
    pub(super) truncated: bool,
}

/// 【Git预览】【差异解析】解析增删行号与代码内容，忽略 Git 路径头并限制行数。
/// 参数: patch 为统一差异，truncated 为接口截断状态；返回: 可直接供界面使用的行模型。
pub(super) fn parse(patch: &str, truncated: bool) -> PreviewContent {
    let mut result = PreviewContent {
        truncated,
        ..Default::default()
    };
    let (mut old, mut new, mut in_hunk) = (0usize, 0usize, false);
    for line in patch.lines() {
        if result.lines.len() >= MAX_LINES {
            result.truncated = true;
            break;
        }
        if line.starts_with("diff --git ") {
            in_hunk = false;
            continue;
        }
        if line.starts_with("@@ ") {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            old = fields
                .get(1)
                .and_then(|value| start_line(value))
                .unwrap_or(0);
            new = fields
                .get(2)
                .and_then(|value| start_line(value))
                .unwrap_or(0);
            in_hunk = true;
            result.lines.push(PatchLine {
                old: None,
                new: None,
                kind: LineKind::Hunk,
                text: format!(
                    "@@ {} {} @@",
                    safe_text(fields.get(1).copied().unwrap_or("-0,0")),
                    safe_text(fields.get(2).copied().unwrap_or("+0,0"))
                ),
            });
            continue;
        }
        if line.starts_with("Binary files ") || line.starts_with("GIT binary patch") {
            result.lines.push(PatchLine {
                old: None,
                new: None,
                kind: LineKind::Notice,
                text: "Binary file changed; no text preview".into(),
            });
        } else if in_hunk {
            let (kind, old_line, new_line) = match line.as_bytes().first() {
                Some(b'+') => {
                    let number = new;
                    new = new.saturating_add(1);
                    result.added += 1;
                    (LineKind::Added, None, Some(number))
                }
                Some(b'-') => {
                    let number = old;
                    old = old.saturating_add(1);
                    result.removed += 1;
                    (LineKind::Removed, Some(number), None)
                }
                Some(b' ') => {
                    let numbers = (old, new);
                    old = old.saturating_add(1);
                    new = new.saturating_add(1);
                    (LineKind::Context, Some(numbers.0), Some(numbers.1))
                }
                _ => continue,
            };
            result.lines.push(PatchLine {
                old: old_line,
                new: new_line,
                kind,
                text: safe_text(&line[1..].replace('\t', "    ")),
            });
        }
    }
    if result.lines.is_empty() {
        result.lines.push(PatchLine {
            old: None,
            new: None,
            kind: LineKind::Notice,
            text: if patch.is_empty() {
                "No text changes in this comparison"
            } else {
                "Metadata or binary change; no text preview"
            }
            .into(),
        });
    }
    result
}

/// 【Git预览】【行号读取】读取差异范围中的起始行号，忽略范围长度。
/// 参数: value 为带正负前缀的范围；返回: 有效起始行号。
fn start_line(value: &str) -> Option<usize> {
    value.get(1..)?.split(',').next()?.parse().ok()
}
