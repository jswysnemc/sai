use std::fmt::Write;

#[derive(Clone, Copy)]
enum DiffKind {
    Context,
    Added,
    Removed,
}

struct DiffOp<'a> {
    kind: DiffKind,
    text: &'a str,
    old_line: Option<usize>,
    new_line: Option<usize>,
}

/// 根据文件写入前后的真实内容生成最小行级 unified diff。
///
/// 参数:
/// - `path`: 目标文件路径
/// - `old_content`: 修改前文件内容
/// - `new_content`: 修改后文件内容
///
/// 返回:
/// - 带三行上下文的 unified diff；内容没有变化时返回空字符串
pub(crate) fn unified_diff(path: &str, old_content: &str, new_content: &str) -> String {
    let old_lines: Vec<&str> = old_content.lines().collect();
    let new_lines: Vec<&str> = new_content.lines().collect();
    let operations = diff_operations(&old_lines, &new_lines);
    let changed: Vec<usize> = operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            matches!(operation.kind, DiffKind::Added | DiffKind::Removed).then_some(index)
        })
        .collect();
    if changed.is_empty() {
        return String::new();
    }

    let ranges = hunk_ranges(&changed, operations.len(), 3);
    let mut output = String::new();
    let _ = writeln!(output, "diff --git a/{path} b/{path}");
    if old_lines.is_empty() {
        output.push_str("--- /dev/null\n");
    } else {
        let _ = writeln!(output, "--- a/{path}");
    }
    let _ = writeln!(output, "+++ b/{path}");
    for (start, end) in ranges {
        let old_start = operations[start]
            .old_line
            .or_else(|| {
                operations[..start]
                    .iter()
                    .rev()
                    .find_map(|op| op.old_line.map(|line| line + 1))
            })
            .unwrap_or(1);
        let new_start = operations[start]
            .new_line
            .or_else(|| {
                operations[..start]
                    .iter()
                    .rev()
                    .find_map(|op| op.new_line.map(|line| line + 1))
            })
            .unwrap_or(1);
        let old_count = operations[start..end]
            .iter()
            .filter(|operation| operation.old_line.is_some())
            .count();
        let new_count = operations[start..end]
            .iter()
            .filter(|operation| operation.new_line.is_some())
            .count();
        let _ = writeln!(
            output,
            "@@ -{old_start},{old_count} +{new_start},{new_count} @@"
        );
        for operation in &operations[start..end] {
            match operation.kind {
                DiffKind::Context => {
                    let _ = writeln!(output, " {text}", text = operation.text);
                }
                DiffKind::Added => {
                    let _ = writeln!(output, "+{text}", text = operation.text);
                }
                DiffKind::Removed => {
                    let _ = writeln!(output, "-{text}", text = operation.text);
                }
            }
        }
    }
    output
}

/// 统计新旧文本的增删行数。
///
/// 与 [`unified_diff`] 走同一套 LCS，因此数字与 diff 正文里实际画出的
/// `+` / `-` 行一致：增删条数只由 LCS 长度决定，与回溯时的取舍无关。
///
/// 参数:
/// - `old_content`: 修改前内容
/// - `new_content`: 修改后内容
///
/// 返回:
/// - `(新增行数, 删除行数)`
pub(crate) fn diff_line_counts(old_content: &str, new_content: &str) -> (usize, usize) {
    let old_lines: Vec<&str> = old_content.lines().collect();
    let new_lines: Vec<&str> = new_content.lines().collect();
    let mut added = 0usize;
    let mut removed = 0usize;
    for operation in diff_operations(&old_lines, &new_lines) {
        match operation.kind {
            DiffKind::Added => added += 1,
            DiffKind::Removed => removed += 1,
            DiffKind::Context => {}
        }
    }
    (added, removed)
}

/// 使用 LCS 回溯得到稳定的增删上下文序列。
fn diff_operations<'a>(old_lines: &[&'a str], new_lines: &[&'a str]) -> Vec<DiffOp<'a>> {
    let mut lcs = vec![vec![0usize; new_lines.len() + 1]; old_lines.len() + 1];
    for old_index in (0..old_lines.len()).rev() {
        for new_index in (0..new_lines.len()).rev() {
            lcs[old_index][new_index] = if old_lines[old_index] == new_lines[new_index] {
                lcs[old_index + 1][new_index + 1] + 1
            } else {
                lcs[old_index + 1][new_index].max(lcs[old_index][new_index + 1])
            };
        }
    }
    let mut operations = Vec::new();
    let (mut old_index, mut new_index) = (0, 0);
    while old_index < old_lines.len() || new_index < new_lines.len() {
        if old_index < old_lines.len()
            && new_index < new_lines.len()
            && old_lines[old_index] == new_lines[new_index]
        {
            operations.push(DiffOp {
                kind: DiffKind::Context,
                text: old_lines[old_index],
                old_line: Some(old_index + 1),
                new_line: Some(new_index + 1),
            });
            old_index += 1;
            new_index += 1;
        } else if new_index == new_lines.len()
            || (old_index < old_lines.len()
                && lcs[old_index + 1][new_index] >= lcs[old_index][new_index + 1])
        {
            operations.push(DiffOp {
                kind: DiffKind::Removed,
                text: old_lines[old_index],
                old_line: Some(old_index + 1),
                new_line: None,
            });
            old_index += 1;
        } else {
            operations.push(DiffOp {
                kind: DiffKind::Added,
                text: new_lines[new_index],
                old_line: None,
                new_line: Some(new_index + 1),
            });
            new_index += 1;
        }
    }
    operations
}

/// 将相邻变更合并为带固定上下文的 hunk 范围。
fn hunk_ranges(changed: &[usize], operation_count: usize, context: usize) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    for &changed_index in changed {
        let start = changed_index.saturating_sub(context);
        let end = (changed_index + context + 1).min(operation_count);
        if let Some((_, previous_end)) = ranges.last_mut() {
            if start <= *previous_end {
                *previous_end = (*previous_end).max(end);
                continue;
            }
        }
        ranges.push((start, end));
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::{diff_line_counts, unified_diff};

    /// 增删条数与 unified diff 里画出的 +/- 行一致。
    #[test]
    fn line_counts_match_the_rendered_diff() {
        assert_eq!(
            diff_line_counts("one\ntwo\nthree\n", "one\nTWO\nthree\n"),
            (1, 1)
        );
        assert_eq!(diff_line_counts("", "hello\nworld\n"), (2, 0));
        assert_eq!(diff_line_counts("a\nb\n", ""), (0, 2));
        assert_eq!(diff_line_counts("same\n", "same\n"), (0, 0));
        // 整行重写：两边都算实际行数，而不是只报净增量
        assert_eq!(diff_line_counts("a\nb\nc\n", "x\ny\nz\n"), (3, 3));
    }

    #[test]
    fn emits_only_changed_region_with_original_context() {
        let diff = unified_diff("notes.txt", "one\ntwo\nthree\n", "one\nTWO\nthree\n");
        assert!(diff.contains("-two"));
        assert!(diff.contains("+TWO"));
        assert!(diff.contains(" one"));
        assert!(!diff.contains("+one"));
    }

    #[test]
    fn emits_new_file_from_empty_original() {
        let diff = unified_diff("new.txt", "", "hello\nworld\n");
        assert!(diff.contains("--- /dev/null"));
        assert!(diff.contains("+hello"));
        assert!(diff.contains("+world"));
    }
}
