use super::grouping::{group_model_stats, group_provider_stats, UsageGroupStats};
use super::query::{
    filter_records, range_start, UsageStatsQuery, DEFAULT_LOG_LIMIT, MAX_LOG_LIMIT,
};
use super::record::{read_records, usage_dir, UsageRecord};
use super::summary::{summarize, UsageSummary};
use super::trend::{build_trend, UsageTrendPoint};
use crate::paths::SaiPaths;
use anyhow::Result;
use serde::Serialize;

/// 用量统计 API 响应。
#[derive(Debug, Clone, Serialize)]
pub struct UsageStatsResponse {
    pub summary: UsageSummary,
    pub trend: Vec<UsageTrendPoint>,
    pub logs: Vec<UsageRecord>,
    pub provider_stats: Vec<UsageGroupStats>,
    pub model_stats: Vec<UsageGroupStats>,
    pub total_logs: usize,
    pub skipped_records: usize,
}

/// 查询聚合统计。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `query`: 筛选与分页条件
///
/// 返回:
/// - 汇总、趋势、日志与分组统计
pub fn get_stats(paths: &SaiPaths, query: UsageStatsQuery) -> Result<UsageStatsResponse> {
    // 1. 按范围下界跳过整月过旧的日志文件，再逐条过滤
    let start = range_start(&query.range);
    let (records, skipped_records) = read_records(&usage_dir(paths), start)?;
    let filtered = filter_records(records, &query);
    let total_logs = filtered.len();
    // 2. 各维度聚合在同一份过滤结果上计算，保证口径一致
    let summary = summarize(&filtered);
    let trend = build_trend(&filtered, &query.range);
    let provider_stats = group_provider_stats(&filtered);
    let model_stats = group_model_stats(&filtered);
    // 3. 日志明细按分页截取
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(DEFAULT_LOG_LIMIT).min(MAX_LOG_LIMIT);
    let logs = filtered.into_iter().skip(offset).take(limit).collect();
    Ok(UsageStatsResponse {
        summary,
        trend,
        logs,
        provider_stats,
        model_stats,
        total_logs,
        skipped_records,
    })
}
