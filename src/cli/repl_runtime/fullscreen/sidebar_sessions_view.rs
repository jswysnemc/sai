//! 会话导航列表渲染：对齐二级网格基线、精简当前态信号并按时间分段组织历史记录。

use super::sidebar::row;
use super::sidebar_action::{SidebarAction, SidebarSection};
use super::sidebar_palette::{fg, SidebarPalette};
use super::sidebar_sessions::{SessionEntry, SessionStatus, SidebarSessions};
use super::sidebar_sessions_group::{compact_relative_time, group_recent_entries};
use super::sidebar_view::SidebarRow;
use super::sidebar_widgets::{badge, columns, heading_with_tone, item, safe_text};
use crate::i18n::text as t;
use std::hash::{Hash, Hasher};

/// 【终端侧栏】【会话动作】计算会话条目的稳定标识键，避免排序变化导致点击错位。
/// 参数: entry 为已定位会话条目；返回: 基于状态目录路径的哈希键。
pub(super) fn key(entry: &SessionEntry) -> u64 {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    entry.state_dir.hash(&mut hash);
    hash.finish()
}

/// 【终端侧栏】【会话列表】固定展示当前会话，并按运行中、就绪与时间分段组织其余会话。
/// 参数: sessions 为会话快照，collapsed 为折叠状态，width 为面板净宽；返回: 会话分组行列表。
pub(super) fn rows(sessions: &SidebarSessions, collapsed: bool, width: usize) -> Vec<SidebarRow> {
    let has_others = sessions.entries.iter().any(|entry| {
        !sessions
            .current
            .as_ref()
            .is_some_and(|current| current.state_dir == entry.state_dir)
    });
    let empty = sessions.current.is_none() && sessions.entries.is_empty();
    let total = sessions
        .entries
        .len()
        .max(usize::from(sessions.current.is_some()));
    let mut rows = vec![row(
        heading_with_tone(
            t("Sessions", "会话"),
            &total.to_string(),
            collapsed,
            empty,
            width,
        ),
        Some(SidebarAction::Toggle(SidebarSection::Sessions)),
    )];
    if let Some(current) = &sessions.current {
        rows.push(session_row(current, sessions.viewed_id(), true, width));
    }
    if collapsed || !has_others {
        return rows;
    }

    // 1. 【终端侧栏】【活跃优先】后台运行中与在线空闲会话排在历史归档之前，小标题对齐第 4 列正文轴
    for (status, label) in [
        (SessionStatus::Running, t("Running", "运行中")),
        (SessionStatus::Idle, t("Ready", "就绪")),
    ] {
        let entries = filter_non_current(sessions, status);
        if entries.is_empty() {
            continue;
        }
        rows.push(subheading(label, width));
        for entry in entries {
            rows.push(session_row(entry, sessions.viewed_id(), false, width));
        }
    }

    // 2. 【终端侧栏】【时间分段】将近期归档会话按今天、近 7 天与更早分段展示，省略冗余子计数
    let recent_entries = filter_non_current(sessions, SessionStatus::Recent);
    for (bucket, bucket_entries) in group_recent_entries(&recent_entries) {
        rows.push(subheading(bucket.label(), width));
        for entry in bucket_entries {
            rows.push(session_row(entry, sessions.viewed_id(), false, width));
        }
    }
    rows
}

/// 【终端侧栏】【状态筛选】提取指定状态且不属于当前固定会话的条目引用。
/// 参数: sessions 为会话集合，status 为目标状态；返回: 匹配的会话引用列表。
fn filter_non_current(sessions: &SidebarSessions, status: SessionStatus) -> Vec<&SessionEntry> {
    sessions
        .entries
        .iter()
        .filter(|entry| {
            entry.status == status
                && !sessions
                    .current
                    .as_ref()
                    .is_some_and(|current| current.state_dir == entry.state_dir)
        })
        .collect()
}

/// 【终端侧栏】【分段小标题】生成与会话标题左边缘对齐的轻量分类行，不附带冗余计数。
/// 参数: label 为分段名称，width 为面板净宽；返回: 只读分类标题行。
fn subheading(label: &str, width: usize) -> SidebarRow {
    let palette = SidebarPalette::current();
    row(
        item(
            &format!("    {}{label}\x1b[39m", fg(palette.subtle)),
            width,
            true,
        ),
        None,
    )
}

/// 【终端侧栏】【会话条目】统一第 2 列状态符与第 4 列标题基线，当前会话仅使用实心状态点。
/// 参数: entry 为会话条目，viewed 为当前浏览会话标识，current 为是否当前终端归属会话，width 为净宽；返回: 可点击会话行。
fn session_row(
    entry: &SessionEntry,
    viewed: Option<&str>,
    current: bool,
    width: usize,
) -> SidebarRow {
    let palette = SidebarPalette::current();
    // 1. 【终端侧栏】【当前状态】当前会话使用实心点，标题缩进与其他会话保持一致
    let (prefix, right_badge) = if current {
        let right = if entry.status == SessionStatus::Running {
            badge(t("running", "运行中"), palette.accent)
        } else {
            String::new()
        };
        (format!("  {}●\x1b[39m ", fg(palette.added)), right)
    } else {
        let (glyph, glyph_color, right) = match entry.status {
            SessionStatus::Running => (
                "◐",
                palette.accent,
                badge(t("running", "运行中"), palette.accent),
            ),
            SessionStatus::Idle => ("○", palette.added, badge(t("ready", "就绪"), palette.added)),
            SessionStatus::Recent => (
                "·",
                palette.subtle,
                badge(&compact_relative_time(&entry.updated_at), palette.subtle),
            ),
        };
        (format!("  {}{glyph}\x1b[39m ", fg(glyph_color)), right)
    };
    let clean_title = safe_text(&entry.title);
    let left = format!("{prefix}{clean_title}");
    let mut result = row(
        item(&columns(&left, &right_badge, width), width, false),
        Some(SidebarAction::Session(key(entry))),
    );
    result.selected = viewed == Some(entry.id.as_str());
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::ansi_clip::strip_ansi;

    /// 【会话分类测试】【条目夹具】构造指定标识与状态的独立会话条目。
    /// 参数: id 为会话标识，status 为会话状态；返回: 测试用会话条目。
    fn entry(id: &str, status: SessionStatus) -> SessionEntry {
        SessionEntry {
            id: id.into(),
            title: id.into(),
            state_dir: std::path::PathBuf::from(id),
            updated_at: String::new(),
            status,
        }
    }

    /// 【会话分类测试】【展示顺序】验证当前固定、运行中优先、就绪及时间分段顺序且不重复展示。
    /// 参数: 无；返回: 无。
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
        let current_plain = strip_ansi(&rows[position("CURRENT")].text);
        assert!(current_plain.starts_with("  ● CURRENT"), "{current_plain}");
        assert!(!current_plain.contains('▎'), "{current_plain}");
        assert!(!current_plain.contains("ready"), "{current_plain}");
        assert_eq!(super::rows(&sessions, true, 40).len(), 2);
    }

    /// 【会话分类测试】【时间分桶】验证历史会话按今天、近 7 天和更早分段且无右侧冗余计数。
    /// 参数: 无；返回: 无。
    #[test]
    fn recent_sessions_split_into_clean_time_buckets() {
        assert!(!compact_relative_time("").is_empty());
        let two_hours_ago = (chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
        assert_eq!(compact_relative_time(&two_hours_ago), "2h");

        let mut sessions = SidebarSessions::default();
        sessions.current = Some(entry("CUR", SessionStatus::Idle));
        let mut today = entry("TODAY-ITEM", SessionStatus::Recent);
        today.updated_at = two_hours_ago;
        let mut week = entry("WEEK-ITEM", SessionStatus::Recent);
        week.updated_at = (chrono::Utc::now() - chrono::Duration::days(3)).to_rfc3339();
        let mut old = entry("OLD-ITEM", SessionStatus::Recent);
        old.updated_at = (chrono::Utc::now() - chrono::Duration::days(14)).to_rfc3339();
        sessions.entries = vec![today, week, old];

        let rendered = rows(&sessions, false, 36)
            .into_iter()
            .map(|r| strip_ansi(&r.text))
            .collect::<Vec<_>>();
        let joined = rendered.join("\n");
        let expected_today = t("Today", "今天");
        let expected_week = t("Past 7d", "近 7 天");
        let expected_older = t("Older", "更早");
        assert!(
            joined.contains(&format!("    {expected_today}")),
            "{joined}"
        );
        assert!(joined.contains(&format!("    {expected_week}")), "{joined}");
        assert!(
            joined.contains(&format!("    {expected_older}")),
            "{joined}"
        );
        let today_header = rendered
            .iter()
            .find(|line| line.contains(expected_today))
            .unwrap();
        assert_eq!(today_header.trim(), expected_today);
    }
}
