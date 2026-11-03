//! 执行结果与只读预览共享变更计划，TUI 和 Web 不重新猜测实际文件变化。

use super::prepare::Plan;
use crate::tools::{
    file_change_model::{AppliedPatch, FileChange},
    file_diff,
};
use anyhow::Result;
use serde_json::json;

/// 【文件补丁】【变更摘要】返回现有界面可识别的 changed_files 和 unified diff。
/// 参数: plan 为实际执行计划；返回: 工具结果 JSON。
pub(super) fn output(plan: &Plan) -> Result<String> {
    let mut diff = String::new();
    let mut files = Vec::new();
    for change in &plan.changes {
        let path = change.path.display().to_string();
        let (added, removed) = file_diff::diff_line_counts(&change.old, &change.new);
        let body = file_diff::unified_diff(&path, &change.old, &change.new);
        let source = change.source.as_ref().unwrap_or(&change.path).display();
        diff.push_str(&format!("diff --git a/{source} b/{path}\n"));
        if change.source.is_some() {
            diff.push_str(&format!("rename from {source}\nrename to {path}\n"));
        }
        // 1. 【文件补丁】【文件语义】文件是否新增或删除由操作决定，不能根据内容是否为空推断
        if change.action == "Added" {
            diff.push_str("--- /dev/null\n");
        } else {
            diff.push_str(&format!("--- a/{source}\n"));
        }
        if change.action == "Deleted" {
            diff.push_str("+++ /dev/null\n");
        } else {
            diff.push_str(&format!("+++ b/{path}\n"));
        }
        if let Some(hunks) = body.splitn(4, '\n').nth(3) {
            diff.push_str(hunks);
        }
        files.push(json!({"action":change.action, "path":path, "source_path":change.source, "added":added, "removed":removed}));
    }
    Ok(serde_json::to_string(
        &json!({"ok":true,"mode":"patch","changed_files":files,"diff":diff}),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【文件补丁测试】【差异文件头】移动保留源路径，清空文件不标为删除，空文件操作保留元数据。
    /// 参数: 无；返回: 无。
    #[test]
    fn apply_patch_headers_preserve_operations_independently_of_content() {
        let root = tempfile::tempdir().unwrap();
        for name in ["old", "clear", "empty"] {
            std::fs::write(
                root.path().join(name),
                if name == "empty" { "" } else { "before\n" },
            )
            .unwrap();
        }
        crate::runtime_cwd::with_directory(root.path().into(), || {
            let operations = super::super::parser::parse("*** Begin Patch\n*** Update File: old\n*** Move to: moved\n@@\n-before\n+after\n*** Update File: clear\n@@\n-before\n*** Add File: added\n*** Delete File: empty\n*** End Patch").unwrap();
            let plan = super::super::prepare::prepare(&operations).unwrap();
            let report: serde_json::Value = serde_json::from_str(&output(&plan).unwrap()).unwrap();
            let blocks = report["diff"]
                .as_str()
                .unwrap()
                .split("diff --git ")
                .skip(1)
                .collect::<Vec<_>>();
            let header = |prefix: &str, name: &str| {
                format!("{prefix}{}\n", root.path().join(name).display())
            };
            assert!(blocks[0].contains(&(header("--- a/", "old") + &header("+++ b/", "moved"))));
            assert!(blocks[1].contains(&(header("--- a/", "clear") + &header("+++ b/", "clear"))));
            assert!(
                blocks[2].contains(&("--- /dev/null\n".to_string() + &header("+++ b/", "added")))
            );
            assert!(blocks[3].contains(&(header("--- a/", "empty") + "+++ /dev/null\n")));
        });
    }
}

/// 【文件补丁】【审批预览】从已验证计划构建统一文件变更模型，不产生写入。
/// 参数: plan 为只读计划；返回: 可供终端渲染的变更集合。
pub(super) fn preview(plan: &Plan) -> AppliedPatch {
    AppliedPatch {
        changes: plan
            .changes
            .iter()
            .map(|change| match change.action {
                "Added" => FileChange::Add {
                    path: change.path.clone(),
                    content: change.new.clone(),
                },
                "Deleted" => FileChange::Delete {
                    path: change.path.clone(),
                    content: change.old.clone(),
                },
                _ => FileChange::Update {
                    path: change.source.as_ref().unwrap_or(&change.path).clone(),
                    move_path: change.source.as_ref().map(|_| change.path.clone()),
                    new_content: change.new.clone(),
                    lines: file_diff::line_changes(&change.old, &change.new),
                },
            })
            .collect(),
    }
}
