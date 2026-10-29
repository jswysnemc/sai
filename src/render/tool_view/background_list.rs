//! 后台查询卡片：调用状态与任务状态分离，短标识只用于展示。

use super::{background::state_label, result_card, ToolView};
use crate::render::{
    render_expand, status_style::ToolHealth, terminal_text as t, ToolCallDisplayMode,
};
use serde_json::Value;

/// 【终端】【后台标识】以唯一前缀显示长标识，碰撞时延长前缀。
/// 参数: `id` 为完整标识，`ids` 为同组标识；返回: 可区分的短标识。
pub(super) fn short_id(id: &str, ids: &[&str]) -> String {
    let length = id.chars().count();
    for count in 8..length {
        let prefix = id.chars().take(count).collect::<String>();
        if ids
            .iter()
            .filter(|other| other.starts_with(&prefix))
            .count()
            <= 1
        {
            return format!("{prefix}…");
        }
    }
    id.to_string()
}

/// 【终端】【后台查询】查询成功只说明记录已获取，不声称后台任务完成。
/// 参数: 工具视图、模式、结果和任务记录；返回: 摘要及可展开的任务列表。
pub(super) fn render(
    view: &ToolView,
    mode: ToolCallDisplayMode,
    value: &Value,
    tasks: &[Value],
) -> String {
    let success = view.outcome.as_ref().is_some_and(|outcome| outcome.ok) && value["ok"] != false;
    let running = tasks
        .iter()
        .filter(|task| task["status"] == "running")
        .count();
    let failed = tasks
        .iter()
        .filter(|task| state_label(task).1 == ToolHealth::Err)
        .count();
    let title = format!(
        "{} · {} {} · {} {} · {} {}",
        if success {
            t("Listed background tasks", "已查询后台任务")
        } else {
            t("Background query failed", "后台查询失败")
        },
        tasks.len(),
        t("tasks", "项"),
        running,
        t("running", "运行中"),
        failed,
        t("failed", "失败")
    );
    let ids = tasks
        .iter()
        .filter_map(|task| task["id"].as_str())
        .collect::<Vec<_>>();
    let expanded = render_expand::expand_override();
    let mut rows = Vec::new();
    for task in tasks.iter().take(if expanded { usize::MAX } else { 5 }) {
        let id = task["id"].as_str().unwrap_or_default();
        let id = if expanded {
            id.to_string()
        } else {
            short_id(id, &ids)
        };
        let command =
            crate::render::ansi_clip::strip_ansi(task["command"].as_str().unwrap_or_default());
        rows.push(format!("{id} · {}\n  $ {command}", state_label(task).0));
    }
    if !expanded && tasks.len() > 5 {
        rows.push(format!(
            "{} {}",
            tasks.len() - 5,
            t(
                "more tasks · expand for IDs",
                "项任务已折叠，展开查看完整标识"
            )
        ));
    }
    if !success {
        if let Some(error) = result_card::text(value, "error") {
            rows.push(error);
        }
    }
    rows.push(
        t(
            "/ps to inspect logs or stop tasks",
            "在 /ps 查看日志或停止任务",
        )
        .into(),
    );
    result_card::render(
        view,
        mode,
        &title,
        if success {
            ToolHealth::Ok
        } else {
            ToolHealth::Err
        },
        &rows.join("\n"),
    )
}
