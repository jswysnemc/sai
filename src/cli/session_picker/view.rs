use super::model::Picker;
use crate::i18n::text as t;
use crate::render::terminal_rows::paint_changed_rows;
use anyhow::Result;
use std::io::{self, Write};

/// 【会话选择】【分组展示】生成工作区标题和会话行，返回选中行位置。
/// 参数: picker 为状态；返回: 视觉行和选中行
pub(super) fn body(picker: &Picker) -> (Vec<String>, usize) {
    let mut lines = Vec::new();
    let mut workspace = None;
    let mut selected = 0;
    for (position, index) in picker.visible.iter().enumerate() {
        let target = &picker.targets[*index];
        if workspace != Some(&target.session.workspace_id) {
            let path = target
                .workspace_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| {
                    format!(
                        "{} ({})",
                        t("Path unavailable", "路径未记录"),
                        target.session.workspace_id
                    )
                });
            lines.push(format!(
                "{} {path}",
                if target.session.workspace_id == picker.current_workspace {
                    "[CWD]"
                } else {
                    "[DIR]"
                }
            ));
            workspace = Some(&target.session.workspace_id);
        }
        if position == picker.selected {
            selected = lines.len();
        }
        lines.push(format!(
            " {} {} {}  {}  {}",
            if position == picker.selected {
                ">"
            } else {
                " "
            },
            if target.session.is_current { "*" } else { "-" },
            target.session.info.title,
            crate::control_commands::relative_time(&target.session.info.updated_at),
            target.session.info.id
        ));
    }
    if lines.is_empty() {
        lines.push(
            t(
                "No matching sessions. Tab switches scope.",
                "没有匹配会话，按 Tab 切换范围。",
            )
            .into(),
        );
    }
    (lines, selected)
}

/// 【会话选择】【绘制】按照终端尺寸显示列表和完整定位信息。
/// 参数: stdout 为终端，picker 为状态，size 为列行数；返回: 输出结果
pub(super) fn draw(stdout: &mut io::Stdout, picker: &Picker, size: (u16, u16)) -> Result<()> {
    let (cols, rows) = size;
    let (body, selected) = body(picker);
    let height = usize::from(rows.saturating_sub(5)).max(1);
    let start = selected
        .saturating_sub(height.saturating_sub(1))
        .min(body.len().saturating_sub(height));
    let scope = if picker.all {
        t("All workspaces", "全部工作区")
    } else {
        t("Current workspace", "当前工作区")
    };
    let mut lines = vec![
        format!("Resume · {scope} ({})", picker.visible.len()),
        format!("{}: {}", t("Search", "搜索"), picker.query),
    ];
    lines.extend(body.into_iter().skip(start).take(height));
    lines.resize(usize::from(rows.saturating_sub(3)), String::new());
    if let Some(target) = picker.target() {
        let instances = crate::runner::session_instances(&target.session.state_dir);
        let owner = instances
            .first()
            .map(|record| record.owner.as_str())
            .unwrap_or("-");
        lines.push(format!(
            "{} · {} · {owner}",
            target.session.info.id, target.session.info.updated_at
        ));
        lines.push(
            target
                .workspace_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| {
                    t(
                        "Use --workspace to locate this session",
                        "请通过 --workspace 指定此会话目录",
                    )
                    .into()
                }),
        );
    } else {
        lines.extend([String::new(), String::new()]);
    }
    lines.push(
        if cols < 70 {
            t(
                "Tab scope · Enter resume · Esc cancel",
                "Tab 范围 · Enter 恢复 · Esc 取消",
            )
        } else {
            t(
                "Tab scope · Type search · Up/Down · Enter resume · Esc cancel",
                "Tab 切换范围 · 输入搜索 · 上下选择 · Enter 恢复 · Esc 取消",
            )
        }
        .into(),
    );
    lines.truncate(usize::from(rows));
    paint_changed_rows(stdout, 0, usize::from(cols), &lines, None)?;
    stdout.flush()?;
    Ok(())
}
