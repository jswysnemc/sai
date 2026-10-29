use super::{result_card, ToolView};
use crate::render::{status_style::ToolHealth, ToolCallDisplayMode};
use serde_json::Value;

/// 【终端】【回收结果】突出原路径、可恢复状态和恢复方式，不重复底层布尔值。
/// 参数: 工具视图及显示模式；返回: 回收站结果卡片，未知输出交给通用视图。
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    if view.name != "trash_path" {
        return None;
    }
    let outcome = view.outcome.as_ref()?;
    let value: Value = serde_json::from_str(&outcome.output).ok()?;
    let path = result_card::text(&value, "original_path")?;
    let success = outcome.ok && value["ok"] != false && value["exists_after"] == false;
    let filename = path.rsplit(['/', '\\']).next().unwrap_or(&path);
    let kind = result_card::text(&value, "kind").unwrap_or_else(|| "item".into());
    let title = if success {
        format!("Trashed {filename} · {kind} · recoverable")
    } else {
        format!("Trash failed · {filename}")
    };
    let mut body = format!("Original: {path}");
    if success {
        let hint = result_card::text(&value, "restore_hint")
            .unwrap_or_else(|| "Open the system Trash to restore this item.".into());
        body.push_str(&format!("\n\x1b[36mRestore: {hint}\x1b[0m"));
        body.push_str("\n\x1b[2mNot permanently deleted.\x1b[0m");
        if let Some(id) = result_card::text(&value, "trash_item_id") {
            body.push_str(&format!("\n\x1b[2mTrash record: {id}\x1b[0m"));
        }
    } else {
        body.push_str("\n\x1b[31mThe original path could not be confirmed as removed.\x1b[0m");
        for key in ["error", "note"] {
            if let Some(text) = result_card::text(&value, key) {
                body.push_str(&format!("\n{text}"));
            }
        }
    }
    Some(result_card::render(
        view,
        mode,
        &title,
        if success {
            ToolHealth::Ok
        } else {
            ToolHealth::Err
        },
        &body,
    ))
}
