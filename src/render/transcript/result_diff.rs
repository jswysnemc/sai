use super::unified_diff::{UnifiedFilePatch, UnifiedLineKind, parse_unified_diffs};
use crate::render::edit_diff::format_diff_stat_status;
use crate::tools::file_change_model::{AppliedPatch, FileChange, LineChange, LineChangeKind};
use serde_json::Value;
use std::path::PathBuf;

/// 编辑工具结果解析出的可渲染 diff 报告。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditResultReport {
    /// 与工具输出同源的 diff 正文（已按 transcript 样式渲染）
    pub rendered: String,
    /// 与正文画出的 `+` / `-` 行一致的 `+N -M` 统计
    pub stats: Option<String>,
}

/// 从编辑工具结果 JSON 解析 diff 报告。
///
/// `write_file` / `str_replace` 把 unified diff 放在输出 JSON 的顶层
/// `diff` 键（`changed_files` 条目内只有 action/path/added/removed 统计）；
/// 条目内嵌 `diff` 的旧格式也兼容。TUI 消费事件时文件可能已经写完，
/// 按参数重建预览会得到空差异，因此定稿快照必须改从结果报告恢复。
///
/// 参数:
/// - `output`: 工具原始输出
///
/// 返回:
/// - 成功输出里的 diff 报告；解析不到时返回空
pub(crate) fn parse_edit_result(output: &str) -> Option<EditResultReport> {
    let value = serde_json::from_str::<Value>(output).ok()?;
    let files = value.get("changed_files")?.as_array()?;
    // 1. 【文件差异】【历史兼容】只有统计的旧结果不能覆盖调用时冻结的差异正文
    if value.get("diff").and_then(Value::as_str).is_none()
        && !files
            .iter()
            .any(|file| file.get("diff").and_then(Value::as_str).is_some())
    {
        return None;
    }
    let mut added = 0usize;
    let mut removed = 0usize;
    let patches = parse_unified_diffs(value.get("diff").and_then(Value::as_str).unwrap_or(""));
    let mut changes = Vec::new();
    let mut single_body = None;
    for file in files {
        added += file.get("added").and_then(Value::as_u64).unwrap_or(0) as usize;
        removed += file.get("removed").and_then(Value::as_u64).unwrap_or(0) as usize;
        let embedded = file
            .get("diff")
            .and_then(Value::as_str)
            .map(parse_unified_diffs);
        let candidates = embedded.as_ref().unwrap_or(&patches);
        let path = file
            .get("path")
            .and_then(Value::as_str)
            .or_else(|| (candidates.len() == 1).then(|| candidates[0].path.as_str()))?;
        let patch = candidates
            .iter()
            .find(|patch| patch.path == path)
            .or_else(|| {
                let matching = candidates
                    .iter()
                    .filter(|patch| {
                        PathBuf::from(&patch.path).ends_with(path)
                            || PathBuf::from(path).ends_with(&patch.path)
                    })
                    .collect::<Vec<_>>();
                (matching.len() == 1).then(|| matching[0])
            });
        if files.len() == 1 && file.get("action").and_then(Value::as_str) != Some("Renamed") {
            single_body = patch.map(crate::render::edit_diff::render_unified_patch_for_transcript);
        }
        changes.push(change_from_result(file, PathBuf::from(path), patch));
    }
    let rendered = single_body
        .unwrap_or_else(|| {
            crate::render::edit_diff::render_patch_preview_for_transcript(&AppliedPatch { changes })
        })
        .trim_end()
        .to_string();
    if rendered.is_empty() {
        return None;
    }
    let stats = Some(format_diff_stat_status(added, removed));
    Some(EditResultReport { rendered, stats })
}

/// 【文件差异】【结果映射】保留每个文件的新增、删除与移动语义，空文件也有明确标题。
/// 参数: file 为变更摘要，path 为目标路径，patch 为对应正文；返回: 单文件展示模型。
fn change_from_result(file: &Value, path: PathBuf, patch: Option<&UnifiedFilePatch>) -> FileChange {
    let text = |added: bool| {
        patch
            .map(|patch| {
                let lines = if added {
                    &patch.new_lines
                } else {
                    &patch.old_lines
                };
                if lines.is_empty() {
                    String::new()
                } else {
                    format!("{}\n", lines.join("\n"))
                }
            })
            .unwrap_or_default()
    };
    match file.get("action").and_then(Value::as_str) {
        Some("Added") => FileChange::Add {
            path,
            content: text(true),
        },
        Some("Deleted") => FileChange::Delete {
            path,
            content: text(false),
        },
        _ => {
            let source = file
                .get("source_path")
                .and_then(Value::as_str)
                .map(PathBuf::from);
            FileChange::Update {
                path: source.as_ref().unwrap_or(&path).clone(),
                move_path: source.map(|_| path),
                new_content: text(true),
                lines: patch
                    .map(|patch| {
                        patch
                            .lines
                            .iter()
                            .map(|line| LineChange {
                                kind: match line.kind {
                                    UnifiedLineKind::Add => LineChangeKind::Add,
                                    UnifiedLineKind::Delete => LineChangeKind::Delete,
                                    UnifiedLineKind::Context => LineChangeKind::Context,
                                },
                                old_line: line.old_line,
                                new_line: line.new_line,
                                text: line.text.clone(),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            }
        }
    }
}
