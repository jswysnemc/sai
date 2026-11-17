//! 历史会话的相对时长格式化与时间分段归桶。

use super::sidebar_sessions::SessionEntry;
use crate::i18n::text as t;

/// 历史会话的时间分段类别。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SessionTimeBucket {
    Today,
    PastWeek,
    Older,
    RecentFallback,
}

impl SessionTimeBucket {
    /// 【终端侧栏】【分段标题】返回当前语言下的时间分段短标题。
    /// 参数: self 为时间分段枚举；返回: 分段标题文本。
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Today => t("Today", "今天"),
            Self::PastWeek => t("Past 7d", "近 7 天"),
            Self::Older => t("Older", "更早"),
            Self::RecentFallback => t("Recent", "最近"),
        }
    }
}

/// 【终端侧栏】【时间解析】解析 RFC3339 或 Unix 秒级时间戳并换算距今秒数。
/// 参数: updated_at 为原始更新时间字符串；返回: 距今非负秒数，为空或格式非法时返回 None。
pub(super) fn elapsed_seconds(updated_at: &str) -> Option<i64> {
    let trimmed = updated_at.trim();
    if trimmed.is_empty() {
        return None;
    }
    let timestamp = chrono::DateTime::parse_from_rfc3339(trimmed)
        .map(|dt| dt.timestamp())
        .or_else(|_| trimmed.parse::<i64>())
        .ok()?;
    Some(chrono::Utc::now().timestamp().saturating_sub(timestamp))
}

/// 【终端侧栏】【相对时间】将更新时间转换为紧凑相对时长，解析失败回退默认词。
/// 参数: updated_at 为原始更新时间文本；返回: 紧凑时长标签。
pub(super) fn compact_relative_time(updated_at: &str) -> String {
    let Some(elapsed) = elapsed_seconds(updated_at) else {
        return t("recent", "最近").into();
    };
    if elapsed < 60 {
        t("now", "刚刚").into()
    } else if elapsed < 3_600 {
        format!("{}m", elapsed / 60)
    } else if elapsed < 86_400 {
        format!("{}h", elapsed / 3_600)
    } else if elapsed < 604_800 {
        format!("{}d", elapsed / 86_400)
    } else {
        format!("{}w", elapsed / 604_800)
    }
}

/// 【终端侧栏】【分段归类】按更新时间距今跨度映射对应时间桶。
/// 参数: updated_at 为原始更新时间文本；返回: 时间分段类别。
pub(super) fn classify_bucket(updated_at: &str) -> SessionTimeBucket {
    match elapsed_seconds(updated_at) {
        Some(elapsed) if elapsed < 86_400 => SessionTimeBucket::Today,
        Some(elapsed) if elapsed < 604_800 => SessionTimeBucket::PastWeek,
        Some(_) => SessionTimeBucket::Older,
        None => SessionTimeBucket::RecentFallback,
    }
}

/// 【终端侧栏】【历史分桶】保持原有时间倒序，将近期会话切分为非空时间段落。
/// 参数: entries 为已排除当前会话的历史条目切片；返回: 按展示顺序排列的分段及各自条目列表。
pub(super) fn group_recent_entries<'a>(
    entries: &[&'a SessionEntry],
) -> Vec<(SessionTimeBucket, Vec<&'a SessionEntry>)> {
    let mut groups: Vec<(SessionTimeBucket, Vec<&'a SessionEntry>)> = Vec::new();
    // 1. 【终端侧栏】【顺序保真】按输入顺序归并相邻桶，既分段降噪又不打乱原有时间排序
    for &entry in entries {
        let bucket = classify_bucket(&entry.updated_at);
        if let Some((_, bucket_entries)) = groups.iter_mut().find(|(b, _)| *b == bucket) {
            bucket_entries.push(entry);
        } else {
            groups.push((bucket, vec![entry]));
        }
    }
    groups
}
