use super::billing::billable_input_tokens;
use super::record::UsageRecord;
use chrono::{Local, TimeZone};
use serde::Serialize;
use std::collections::BTreeMap;

/// 趋势图按时间桶聚合的数据点。
#[derive(Debug, Clone, Default, Serialize)]
pub struct UsageTrendPoint {
    pub date: String,
    pub label: String,
    pub requests: u64,
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// 按缓存系数折算后的等效计费输入量
    pub billable_input_tokens: u64,
}

/// 构建趋势序列。
///
/// 参数:
/// - `records`: 已过滤的记录
/// - `range`: 查询范围标识，决定按小时还是按日聚合
///
/// 返回:
/// - 按时间升序排列的趋势点
pub(crate) fn build_trend(records: &[UsageRecord], range: &str) -> Vec<UsageTrendPoint> {
    // 1. 一天以内按小时聚合可读性更好，其余按自然日
    let hourly = range == "1d" || range == "today";
    let mut buckets: BTreeMap<String, UsageTrendPoint> = BTreeMap::new();
    for record in records {
        let Some(dt) = Local.timestamp_opt(record.created_at, 0).single() else {
            continue;
        };
        let (date, label) = if hourly {
            (
                dt.format("%Y-%m-%d %H:00").to_string(),
                dt.format("%H:00").to_string(),
            )
        } else {
            (
                dt.format("%Y-%m-%d").to_string(),
                dt.format("%m-%d").to_string(),
            )
        };
        // 2. 同一时间桶内累加请求数与两种令牌口径
        let point = buckets
            .entry(date.clone())
            .or_insert_with(|| UsageTrendPoint {
                date,
                label,
                ..UsageTrendPoint::default()
            });
        accumulate_point(point, record);
    }
    buckets.into_values().collect()
}

/// 把单条记录累加进趋势点。
///
/// 参数:
/// - `point`: 目标趋势点
/// - `record`: 待累加的记录
///
/// 返回:
/// - 无；就地修改趋势点
fn accumulate_point(point: &mut UsageTrendPoint, record: &UsageRecord) {
    point.requests = point.requests.saturating_add(1);
    point.total_tokens = point
        .total_tokens
        .saturating_add(record.total_tokens_or_sum());
    point.input_tokens = point
        .input_tokens
        .saturating_add(record.input_tokens.unwrap_or(0));
    point.output_tokens = point
        .output_tokens
        .saturating_add(record.output_tokens.unwrap_or(0));
    point.billable_input_tokens = point
        .billable_input_tokens
        .saturating_add(billable_input_tokens(record));
}
