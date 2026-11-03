//! 精确行级补丁应用；不忽略空白、不模糊匹配、不选择多个命中中的任意一个。

use super::model::{Hunk, MAX_FILE_BYTES};
use crate::tools::file_edit::line_endings::{
    LineEndingStyle, materialize_model_text, to_model_text_view,
};
use anyhow::{Result, ensure};

/// 【文件补丁】【文本应用】顺序应用不重叠片段，保留 BOM、原行尾及末尾换行状态。
/// 参数: raw 为读取快照，hunks 为修改片段；返回: 修改后的完整文本。
pub(super) fn apply(raw: &str, hunks: &[Hunk]) -> Result<String> {
    let bom = raw.starts_with('\u{feff}');
    let view = to_model_text_view(raw.strip_prefix('\u{feff}').unwrap_or(raw));
    ensure!(
        view.line_ending_style != LineEndingStyle::Mixed,
        "patch refuses mixed line endings; use str_replace to preserve exact bytes"
    );
    let old = view.text.lines().map(str::to_string).collect::<Vec<_>>();
    let mut final_newline = view.text.is_empty() || view.text.ends_with('\n');
    let mut output = Vec::new();
    let mut cursor = 0;
    let mut inspected = 0usize;
    for (number, hunk) in hunks.iter().enumerate() {
        inspected = inspected.saturating_add(old.len().saturating_sub(cursor));
        ensure!(
            inspected <= 20_000_000,
            "patch matching exceeds the work limit; combine nearby changes into fewer hunks"
        );
        let mut start = cursor;
        if let Some(anchor) = &hunk.anchor {
            let candidates = (cursor..old.len())
                .filter(|&index| old[index] == *anchor)
                .take(2)
                .collect::<Vec<_>>();
            ensure!(
                candidates.len() == 1,
                "hunk {} anchor must match exactly once; found {}. Read the file again and provide unique context",
                number + 1,
                candidates.len()
            );
            start = candidates[0] + 1;
        }
        let candidates = if hunk.old.is_empty() {
            ensure!(
                hunk.anchor.is_some() || hunk.at_eof || old.is_empty(),
                "hunk {} has no old context; use a unique anchor or *** End of File for insertion",
                number + 1
            );
            vec![if hunk.at_eof { old.len() } else { start }]
        } else {
            positions(&old, &hunk.old, start, hunk.at_eof)
        };
        ensure!(
            candidates.len() == 1,
            "hunk {} context must match exactly once; found {}. File may have changed; read it again and provide unique context",
            number + 1,
            candidates.len()
        );
        let position = candidates[0];
        let end = position + hunk.old.len();
        if hunk.old_no_newline || hunk.new_no_newline {
            ensure!(
                end == old.len(),
                "newline marker is only valid at the end of the file"
            );
        }
        ensure!(
            !hunk.old_no_newline || !view.text.ends_with('\n'),
            "old content has a final newline; patch expected none"
        );
        if hunk.old_no_newline {
            final_newline = true;
        }
        if hunk.new_no_newline {
            final_newline = false;
        }
        output.extend_from_slice(&old[cursor..position]);
        output.extend(hunk.new.iter().cloned());
        cursor = end;
    }
    output.extend_from_slice(&old[cursor..]);
    let mut text = output.join("\n");
    if !output.is_empty() && final_newline {
        text.push('\n');
    }
    if bom {
        text.insert(0, '\u{feff}');
    }
    let text = materialize_model_text(&text, view.line_ending_style);
    ensure!(
        text.len() <= MAX_FILE_BYTES,
        "patched file exceeds the 8 MiB limit"
    );
    Ok(text)
}

/// 【文件补丁】【线性匹配】使用前缀表查找上下文，重复大段文本不产生平方级扫描。
/// 参数: lines 为原文，pattern 为旧片段，start 为起点，eof 为末尾约束；返回: 最多两个命中位置。
fn positions(lines: &[String], pattern: &[String], start: usize, eof: bool) -> Vec<usize> {
    if eof {
        return lines
            .len()
            .checked_sub(pattern.len())
            .filter(|&index| index >= start && lines[index..] == *pattern)
            .into_iter()
            .collect();
    }
    let mut prefix = vec![0; pattern.len()];
    let mut matched = 0;
    for index in 1..pattern.len() {
        while matched > 0 && pattern[index] != pattern[matched] {
            matched = prefix[matched - 1];
        }
        if pattern[index] == pattern[matched] {
            matched += 1;
        }
        prefix[index] = matched;
    }
    let mut found = Vec::new();
    matched = 0;
    for (index, line) in lines.iter().enumerate().skip(start) {
        while matched > 0 && line != &pattern[matched] {
            matched = prefix[matched - 1];
        }
        if line == &pattern[matched] {
            matched += 1;
        }
        if matched == pattern.len() {
            found.push(index + 1 - matched);
            if found.len() == 2 {
                break;
            }
            matched = prefix[matched - 1];
        }
    }
    found
}
