//! 会话导航只负责列表展示，执行状态与当前浏览状态分别标记。

use super::sidebar::row;
use super::sidebar_action::{SidebarAction, SidebarSection};
use super::sidebar_palette::SidebarPalette;
use super::sidebar_sessions::{SessionEntry, SessionStatus, SidebarSessions};
use super::sidebar_view::SidebarRow;
use super::sidebar_widgets::{badge, columns, heading, item, safe_text};
use crate::i18n::text as t;
use std::hash::{Hash, Hasher};

/// 【终端侧栏】【会话动作】参数 entry 为已定位条目；返回不受列表排序变化影响的动作键
pub(super) fn key(entry: &SessionEntry) -> u64 {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    entry.state_dir.hash(&mut hash);
    hash.finish()
}

/// 【终端侧栏】【会话列表】参数为快照、折叠状态与净宽；返回当前和近期会话，优先展示执行会话
pub(super) fn rows(sessions: &SidebarSessions, collapsed: bool, width: usize) -> Vec<SidebarRow> {
    let mut rows = vec![row(
        heading(
            t("Sessions", "会话"),
            &sessions.entries.len().to_string(),
            collapsed,
            width,
        ),
        Some(SidebarAction::Toggle(SidebarSection::Sessions)),
    )];
    if let Some(current) = &sessions.current {
        rows.push(session_row(current, sessions.viewed_id(), true, width));
    }
    if !collapsed {
        // 1. 【终端侧栏】【状态分类】当前会话固定在顶部，其他会话按运行中、就绪、最近顺序展示
        for (status, label) in [
            (SessionStatus::Running, t("Running", "运行中")),
            (SessionStatus::Idle, t("Ready", "就绪")),
            (SessionStatus::Recent, t("Recent", "最近")),
        ] {
            let entries = sessions
                .entries
                .iter()
                .filter(|entry| {
                    entry.status == status
                        && !sessions
                            .current
                            .as_ref()
                            .is_some_and(|current| current.state_dir == entry.state_dir)
                })
                .collect::<Vec<_>>();
            if entries.is_empty() {
                continue;
            }
            rows.push(row(
                item(
                    &columns(label, &entries.len().to_string(), width),
                    width,
                    true,
                ),
                None,
            ));
            for entry in entries {
                rows.push(session_row(entry, sessions.viewed_id(), false, width));
            }
        }
    }
    rows
}

/// 【终端侧栏】【会话条目】参数 entry/viewed/current/width 为条目、浏览标识、执行归属与宽度；返回可点击单行
fn session_row(
    entry: &SessionEntry,
    viewed: Option<&str>,
    current: bool,
    width: usize,
) -> SidebarRow {
    let palette = SidebarPalette::current();
    let (status, color) = match entry.status {
        SessionStatus::Running => (t("running", "运行中"), palette.accent),
        SessionStatus::Idle => (t("ready", "就绪"), palette.added),
        SessionStatus::Recent => (t("recent", "最近"), palette.muted),
    };
    let title = format!(
        "{}{}",
        if current {
            t("Current · ", "当前 · ")
        } else {
            ""
        },
        safe_text(&entry.title)
    );
    let mut result = row(
        item(&columns(&title, &badge(status, color), width), width, false),
        Some(SidebarAction::Session(key(entry))),
    );
    result.selected = viewed == Some(entry.id.as_str());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【会话分类测试】【条目夹具】参数：id 为标识，status 为真实状态；返回：独立会话条目
    fn entry(id: &str, status: SessionStatus) -> SessionEntry {
        SessionEntry {
            id: id.into(),
            title: id.into(),
            state_dir: std::path::PathBuf::from(id),
            updated_at: String::new(),
            status,
        }
    }

    /// 【会话分类测试】【展示顺序】参数：无；返回：无，验证当前固定、运行中优先和就绪与最近分别展示
    #[test]
    fn sessions_group_current_running_ready_and_recent_without_duplicates() {
        let current = entry("CURRENT", SessionStatus::Idle);
        let mut sessions = SidebarSessions::default();
        sessions.current = Some(current.clone());
        sessions.entries = vec![
            entry("RECENT", SessionStatus::Recent),
            entry("READY", SessionStatus::Idle),
            current,
            entry("RUNNING", SessionStatus::Running),
        ];
        let rows = rows(&sessions, false, 40);
        let position = |title: &str| {
            rows.iter()
                .position(|row| row.action.is_some() && row.text.contains(title))
                .unwrap()
        };
        assert!(position("CURRENT") < position("RUNNING"));
        assert!(position("RUNNING") < position("READY"));
        assert!(position("READY") < position("RECENT"));
        assert_eq!(
            rows.iter()
                .filter(|row| row.text.contains("CURRENT"))
                .count(),
            1
        );
        assert_eq!(super::rows(&sessions, true, 40).len(), 2);
    }
}
