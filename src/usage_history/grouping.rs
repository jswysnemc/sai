use super::billing::billable_input_tokens;
use super::record::UsageRecord;
use serde::Serialize;
use std::collections::BTreeMap;

/// 按供应商或模型维度聚合后的统计行。
#[derive(Debug, Clone, Default, Serialize)]
pub struct UsageGroupStats {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub request_count: u64,
    pub success_count: u64,
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// 输入总量中命中缓存读取的部分
    pub cache_read_tokens: u64,
    /// 按缓存系数折算后的等效计费输入量
    pub billable_input_tokens: u64,
    pub average_duration_ms: Option<f64>,
    pub last_used_at: Option<i64>,
}

/// 分组累加器，聚合完成后一次性换算平均耗时。
#[derive(Default)]
struct GroupAcc {
    stats: UsageGroupStats,
    duration_total: u64,
}

/// 按供应商聚合。
///
/// 参数:
/// - `records`: 已过滤的记录
///
/// 返回:
/// - 按总令牌量倒序排列的供应商统计
pub(crate) fn group_provider_stats(records: &[UsageRecord]) -> Vec<UsageGroupStats> {
    group_by(records, |record| {
        let label = if record.provider_name.trim().is_empty() {
            record.provider_id.clone()
        } else {
            record.provider_name.clone()
        };
        GroupKey {
            id: record.provider_id.clone(),
            label,
            model: None,
        }
    })
}

/// 按供应商与模型的组合聚合。
///
/// 参数:
/// - `records`: 已过滤的记录
///
/// 返回:
/// - 按总令牌量倒序排列的模型统计
pub(crate) fn group_model_stats(records: &[UsageRecord]) -> Vec<UsageGroupStats> {
    group_by(records, |record| GroupKey {
        id: format!("{}::{}", record.provider_id, record.model),
        label: record.model.clone(),
        model: Some(record.model.clone()),
    })
}

/// 分组标识与展示信息。
struct GroupKey {
    id: String,
    label: String,
    model: Option<String>,
}

/// 通用分组聚合。
///
/// 参数:
/// - `records`: 已过滤的记录
/// - `key_of`: 从记录派生分组标识的闭包
///
/// 返回:
/// - 按总令牌量倒序排列的统计行
fn group_by(
    records: &[UsageRecord],
    key_of: impl Fn(&UsageRecord) -> GroupKey,
) -> Vec<UsageGroupStats> {
    let mut map: BTreeMap<String, GroupAcc> = BTreeMap::new();
    for record in records {
        let key = key_of(record);
        let acc = map.entry(key.id.clone()).or_insert_with(|| GroupAcc {
            stats: UsageGroupStats {
                id: key.id,
                label: key.label,
                provider_id: Some(record.provider_id.clone()),
                provider_name: Some(record.provider_name.clone()),
                model: key.model,
                ..UsageGroupStats::default()
            },
            duration_total: 0,
        });
        accumulate(acc, record);
    }
    let mut rows: Vec<UsageGroupStats> = map.into_values().map(finish).collect();
    rows.sort_by_key(|row| std::cmp::Reverse(row.total_tokens));
    rows
}

/// 把单条记录累加进分组。
///
/// 参数:
/// - `acc`: 目标分组累加器
/// - `record`: 待累加的记录
///
/// 返回:
/// - 无；就地修改累加器
fn accumulate(acc: &mut GroupAcc, record: &UsageRecord) {
    let stats = &mut acc.stats;
    stats.request_count = stats.request_count.saturating_add(1);
    if record.status == "success" {
        stats.success_count = stats.success_count.saturating_add(1);
    }
    stats.total_tokens = stats
        .total_tokens
        .saturating_add(record.total_tokens_or_sum());
    stats.input_tokens = stats
        .input_tokens
        .saturating_add(record.input_tokens.unwrap_or(0));
    stats.output_tokens = stats
        .output_tokens
        .saturating_add(record.output_tokens.unwrap_or(0));
    stats.cache_read_tokens = stats.cache_read_tokens.saturating_add(record.cache_read());
    stats.billable_input_tokens = stats
        .billable_input_tokens
        .saturating_add(billable_input_tokens(record));
    stats.last_used_at = Some(stats.last_used_at.unwrap_or(0).max(record.created_at));
    acc.duration_total = acc.duration_total.saturating_add(record.duration_ms);
}

/// 收尾分组，换算平均耗时。
///
/// 参数:
/// - `acc`: 累加完成的分组
///
/// 返回:
/// - 可序列化的统计行
fn finish(acc: GroupAcc) -> UsageGroupStats {
    let mut stats = acc.stats;
    if stats.request_count > 0 {
        stats.average_duration_ms = Some(acc.duration_total as f64 / stats.request_count as f64);
    }
    stats
}
