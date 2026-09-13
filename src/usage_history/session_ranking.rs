use super::query::UsageStatsQuery;
use super::record::UsageRecord;
use super::summary::{summarize, UsageSummary};
use crate::paths::SaiPaths;
use serde::Serialize;
use std::collections::BTreeMap;

mod metadata;
#[cfg(test)]
mod tests;

/// 一个会话在筛选范围内的累计用量，与请求日志共用统计口径。
#[derive(Clone, Debug, Serialize)]
pub struct UsageSessionStats {
    pub session_id: String,
    pub workspace_id: Option<String>,
    pub title: Option<String>,
    pub session_available: bool,
    pub last_used_at: Option<i64>,
    #[serde(flatten)]
    pub summary: UsageSummary,
}

/// 【用量统计】【会话排行】按工作区和会话聚合已筛选的调用，排序后截取高消耗条目。
///
/// 参数: `paths` 为应用路径，`records` 为完整筛选结果，`query` 为排行排序与数量
/// 返回: 排行和符合条件的会话总数；没有会话标识的辅助调用不进入排行
pub(super) fn group_session_stats(
    paths: &SaiPaths,
    records: &[UsageRecord],
    query: &UsageStatsQuery,
) -> (Vec<UsageSessionStats>, usize) {
    let metadata = metadata::SessionMetadata::load(paths);
    let mut groups: BTreeMap<(Option<String>, String), Vec<&UsageRecord>> = BTreeMap::new();
    // 1. 工作区参与身份，避免不同项目的 default 会话合并
    for record in records {
        let Some(session_id) = record
            .session_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        else {
            continue;
        };
        let workspace_id = metadata.workspace_id(record, session_id);
        groups
            .entry((workspace_id, session_id.to_string()))
            .or_default()
            .push(record);
    }
    let total_sessions = groups.len();
    let mut rows: Vec<_> = groups
        .into_iter()
        .map(|((workspace_id, session_id), records)| {
            let title = metadata.title(workspace_id.as_deref(), &session_id);
            UsageSessionStats {
                session_available: title.is_some(),
                session_id,
                workspace_id,
                title,
                last_used_at: records.iter().map(|record| record.created_at).max(),
                summary: summarize(records),
            }
        })
        .collect();
    // 2. 排行使用全部匹配记录，不受请求日志分页影响
    rows.sort_by(|left, right| {
        sort_value(right, query.session_sort.as_deref())
            .cmp(&sort_value(left, query.session_sort.as_deref()))
            .then_with(|| left.workspace_id.cmp(&right.workspace_id))
            .then_with(|| left.session_id.cmp(&right.session_id))
    });
    rows.truncate(query.session_limit.unwrap_or(10).clamp(1, 50));
    (rows, total_sessions)
}

/// 【用量统计】【会话排行】选择排序指标，未知值回退到原始总量。
/// 参数: `row` 为会话统计，`sort` 为客户端排序标识
/// 返回: 用于降序排列的计数
fn sort_value(row: &UsageSessionStats, sort: Option<&str>) -> u64 {
    match sort {
        Some("billable_tokens") => row.summary.billable_total_tokens,
        Some("requests") => row.summary.total_requests,
        _ => row.summary.total_tokens,
    }
}
