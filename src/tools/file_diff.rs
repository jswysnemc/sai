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

/// 根据文件写入前后的真实内容生成有界行级 unified diff。
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
    let operations = text_diff_operations(old_content, new_content);
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
    if new_lines.is_empty() {
        output.push_str("+++ /dev/null\n");
    } else {
        let _ = writeln!(output, "+++ b/{path}");
    }
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
        let old_start = old_start.saturating_sub(usize::from(old_count == 0));
        let new_start = new_start.saturating_sub(usize::from(new_count == 0));
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
            let old_eof =
                operation.old_line == Some(old_lines.len()) && !old_content.ends_with('\n');
            let new_eof =
                operation.new_line == Some(new_lines.len()) && !new_content.ends_with('\n');
            if old_eof || new_eof {
                output.push_str("\\ No newline at end of file\n");
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
    let mut added = 0usize;
    let mut removed = 0usize;
    for operation in text_diff_operations(old_content, new_content) {
        match operation.kind {
            DiffKind::Added => added += 1,
            DiffKind::Removed => removed += 1,
            DiffKind::Context => {}
        }
    }
    (added, removed)
}

/// 【文件差异】【末尾换行】把仅末尾换行变化表示为末行替换，摘要与实际差异保持一致。
/// 参数: old/new 为完整文本；返回: 带原行号的差异操作。
fn text_diff_operations<'a>(old: &'a str, new: &'a str) -> Vec<DiffOp<'a>> {
    let old_lines = old.lines().collect::<Vec<_>>();
    let new_lines = new.lines().collect::<Vec<_>>();
    let mut operations = diff_operations(&old_lines, &new_lines);
    if old.ends_with('\n') != new.ends_with('\n')
        && operations.last().is_some_and(|op| {
            matches!(op.kind, DiffKind::Context)
                && op.old_line == Some(old_lines.len())
                && op.new_line == Some(new_lines.len())
        })
    {
        if let Some(last) = operations.pop() {
            operations.push(DiffOp {
                kind: DiffKind::Removed,
                text: last.text,
                old_line: last.old_line,
                new_line: None,
            });
            operations.push(DiffOp {
                kind: DiffKind::Added,
                text: last.text,
                old_line: None,
                new_line: last.new_line,
            });
        }
    }
    operations
}

/// 使用 LCS 回溯得到稳定的增删上下文序列。
fn diff_operations<'a>(old_lines: &[&'a str], new_lines: &[&'a str]) -> Vec<DiffOp<'a>> {
    let prefix = old_lines
        .iter()
        .zip(new_lines)
        .take_while(|(a, b)| a == b)
        .count();
    let suffix = old_lines[prefix..]
        .iter()
        .rev()
        .zip(new_lines[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let old_core = &old_lines[prefix..old_lines.len() - suffix];
    let new_core = &new_lines[prefix..new_lines.len() - suffix];
    let mut result = (0..prefix)
        .map(|index| DiffOp {
            kind: DiffKind::Context,
            text: old_lines[index],
            old_line: Some(index + 1),
            new_line: Some(index + 1),
        })
        .collect::<Vec<_>>();
    let core = if old_core.len().saturating_mul(new_core.len()) > 1_000_000 {
        // 1. 【文件差异】【资源边界】整段重写使用线性空间，避免补丁输出分配平方级矩阵
        old_core
            .iter()
            .enumerate()
            .map(|(index, text)| DiffOp {
                kind: DiffKind::Removed,
                text,
                old_line: Some(index + 1),
                new_line: None,
            })
            .chain(new_core.iter().enumerate().map(|(index, text)| DiffOp {
                kind: DiffKind::Added,
                text,
                old_line: None,
                new_line: Some(index + 1),
            }))
            .collect()
    } else {
        diff_core_operations(old_core, new_core)
    };
    result.extend(core.into_iter().map(|mut op| {
        op.old_line = op.old_line.map(|n| n + prefix);
        op.new_line = op.new_line.map(|n| n + prefix);
        op
    }));
    result.extend((0..suffix).map(|index| DiffOp {
        kind: DiffKind::Context,
        text: old_lines[old_lines.len() - suffix + index],
        old_line: Some(old_lines.len() - suffix + index + 1),
        new_line: Some(new_lines.len() - suffix + index + 1),
    }));
    result
}

/// 【文件差异】【局部比对】只对有界变更区间计算 LCS，行号相对片段起点。
/// 参数: old_lines/new_lines 为前后行；返回: 差异操作。
fn diff_core_operations<'a>(old_lines: &[&'a str], new_lines: &[&'a str]) -> Vec<DiffOp<'a>> {
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

/// 【文件差异】【预览映射】从统一差异算法生成带行号且上下文有界的终端模型。
/// 参数: old/new 为修改前后文本；返回: 每处变更附近三行上下文及增删行。
pub(crate) fn line_changes(old: &str, new: &str) -> Vec<super::file_change_model::LineChange> {
    use super::file_change_model::{LineChange, LineChangeKind};
    let operations = text_diff_operations(old, new);
    let changed = operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            (!matches!(operation.kind, DiffKind::Context)).then_some(index)
        })
        .collect::<Vec<_>>();
    hunk_ranges(&changed, operations.len(), 3)
        .into_iter()
        .flat_map(|(start, end)| {
            operations[start..end].iter().map(|operation| LineChange {
                kind: match operation.kind {
                    DiffKind::Added => LineChangeKind::Add,
                    DiffKind::Removed => LineChangeKind::Delete,
                    DiffKind::Context => LineChangeKind::Context,
                },
                old_line: operation.old_line,
                new_line: operation.new_line,
                text: operation.text.to_string(),
            })
        })
        .collect()
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

    /// 【文件差异测试】【末尾换行】仅换行变化和伴随其他修改时都生成可核对的差异及计数。
    /// 参数: 无；返回: 无。
    #[test]
    fn final_newline_changes_are_visible_in_diff_and_counts() {
        assert_eq!(diff_line_counts("a", "a\n"), (1, 1));
        let diff = unified_diff("a", "old\nlast\n", "new\nlast");
        assert!(diff.contains("-last\n+last\n\\ No newline at end of file\n"), "{diff}");
        assert_eq!(diff_line_counts("old\nlast\n", "new\nlast"), (2, 2));
        assert!(unified_diff("a", "deleted\n", "").contains("+++ /dev/null"));
    }

    /// 【文件差异测试】【大文件】大段重写使用有界计算，增删统计与输出保持一致。
    /// 参数: 无；返回: 无。
    #[test]
    fn large_rewrite_uses_bounded_diff_work() {
        let old = (0..5000).map(|index| format!("old-{index}\n")).collect::<String>();
        let new = (0..5000).map(|index| format!("new-{index}\n")).collect::<String>();
        assert_eq!(diff_line_counts(&old, &new), (5000, 5000));
    }

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
