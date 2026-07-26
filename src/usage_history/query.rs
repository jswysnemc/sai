use super::record::UsageRecord;
use chrono::{Local, TimeZone};
use serde::Deserialize;

pub(crate) const DEFAULT_LOG_LIMIT: usize = 100;
pub(crate) const MAX_LOG_LIMIT: usize = 500;

/// 统计查询参数。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct UsageStatsQuery {
    #[serde(default = "default_range")]
    pub range: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub provider_search: Option<String>,
    #[serde(default)]
    pub model_search: Option<String>,
    #[serde(default)]
    pub offset: Option<usize>,
    #[serde(default)]
    pub limit: Option<usize>,
}

fn default_range() -> String {
    "7d".to_string()
}

/// 按查询条件筛选记录并按时间倒序排列。
///
/// 参数:
/// - `records`: 原始记录
/// - `query`: 筛选条件
///
/// 返回:
/// - 过滤并排序后的记录
pub(crate) fn filter_records(
    mut records: Vec<UsageRecord>,
    query: &UsageStatsQuery,
) -> Vec<UsageRecord> {
    let start = range_start(&query.range);
    let source = normalized_filter(query.source.as_deref());
    let status = normalized_filter(query.status.as_deref());
    let provider_search = normalized_search(query.provider_search.as_deref());
    let model_search = normalized_search(query.model_search.as_deref());
    records.retain(|record| {
        if let Some(start) = start {
            if record.created_at < start {
                return false;
            }
        }
        if let Some(source) = source.as_deref() {
            if record.source != source {
                return false;
            }
        }
        if let Some(status) = status.as_deref() {
            if status == "missing_usage" {
                if record.usage_source != "missing" {
                    return false;
                }
            } else if record.status != status {
                return false;
            }
        }
        if let Some(search) = provider_search.as_deref() {
            let haystack =
                format!("{} {}", record.provider_id, record.provider_name).to_ascii_lowercase();
            if !haystack.contains(search) {
                return false;
            }
        }
        if let Some(search) = model_search.as_deref() {
            if !record.model.to_ascii_lowercase().contains(search) {
                return false;
            }
        }
        true
    });
    records.sort_by_key(|record| std::cmp::Reverse(record.created_at));
    records
}

/// 把范围标识换算成起始时间戳。
///
/// 参数:
/// - `range`: 范围标识，如 today / 1d / 30d / all
///
/// 返回:
/// - 起始时间戳；all 表示不设下界
pub(crate) fn range_start(range: &str) -> Option<i64> {
    let now = Local::now().timestamp();
    match range {
        "today" => Local::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .and_then(|naive| Local.from_local_datetime(&naive).single())
            .map(|dt| dt.timestamp()),
        "1d" => Some(now.saturating_sub(86_400)),
        "30d" => Some(now.saturating_sub(30 * 86_400)),
        "90d" => Some(now.saturating_sub(90 * 86_400)),
        "all" => None,
        _ => Some(now.saturating_sub(7 * 86_400)),
    }
}

fn normalized_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "all")
        .map(str::to_string)
}

fn normalized_search(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}
