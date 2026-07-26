use super::billing::billable_input_tokens;
use super::record::UsageRecord;
use serde::Serialize;

/// 汇总卡片数据。
///
/// `input_tokens` 是供应商上报的输入总量，包含命中缓存的部分。
/// `billable_input_tokens` 按缓存计价系数折算，才是与账单可比的口径；
/// 长会话里缓存命中率极高时，两者可以相差一个数量级。
#[derive(Debug, Clone, Default, Serialize)]
pub struct UsageSummary {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub missing_usage_requests: u64,
    pub provider_reported_requests: u64,
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// 输入总量中命中缓存读取的部分
    pub cache_read_tokens: u64,
    /// 输入总量中写入缓存的部分
    pub cache_write_tokens: u64,
    /// 按缓存系数折算后的等效计费输入量
    pub billable_input_tokens: u64,
    /// 等效计费输入量与输出量之和
    pub billable_total_tokens: u64,
    pub average_duration_ms: Option<f64>,
}

/// 汇总一组记录。
///
/// 参数:
/// - `records`: 已按查询条件过滤的记录
///
/// 返回:
/// - 请求数、令牌量与平均耗时构成的汇总
pub(crate) fn summarize(records: &[UsageRecord]) -> UsageSummary {
    let mut summary = UsageSummary::default();
    let mut duration_total = 0u64;
    for record in records {
        // 1. 请求计数按状态与用量来源分流
        summary.total_requests = summary.total_requests.saturating_add(1);
        if record.status == "success" {
            summary.successful_requests = summary.successful_requests.saturating_add(1);
        } else {
            summary.failed_requests = summary.failed_requests.saturating_add(1);
        }
        if record.usage_source == "missing" {
            summary.missing_usage_requests = summary.missing_usage_requests.saturating_add(1);
        }
        if record.usage_source == "provider_reported" {
            summary.provider_reported_requests =
                summary.provider_reported_requests.saturating_add(1);
        }
        // 2. 原始口径累加
        let output = record.output_tokens.unwrap_or(0);
        summary.total_tokens = summary
            .total_tokens
            .saturating_add(record.total_tokens_or_sum());
        summary.input_tokens = summary
            .input_tokens
            .saturating_add(record.input_tokens.unwrap_or(0));
        summary.output_tokens = summary.output_tokens.saturating_add(output);
        // 3. 计费口径累加，缓存明细单列以便前端解释差异来源
        summary.cache_read_tokens = summary
            .cache_read_tokens
            .saturating_add(record.cache_read());
        summary.cache_write_tokens = summary
            .cache_write_tokens
            .saturating_add(record.cache_write());
        let billable_input = billable_input_tokens(record);
        summary.billable_input_tokens =
            summary.billable_input_tokens.saturating_add(billable_input);
        summary.billable_total_tokens = summary
            .billable_total_tokens
            .saturating_add(billable_input.saturating_add(output));
        duration_total = duration_total.saturating_add(record.duration_ms);
    }
    if summary.total_requests > 0 {
        summary.average_duration_ms = Some(duration_total as f64 / summary.total_requests as f64);
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(input: u64, output: u64, read: u64, write: u64) -> UsageRecord {
        UsageRecord {
            id: "u".to_string(),
            created_at: 0,
            completed_at: 0,
            duration_ms: 100,
            source: "chat".to_string(),
            operation: "turn".to_string(),
            provider_id: "anthropic".to_string(),
            provider_name: "Anthropic".to_string(),
            model: "claude-opus".to_string(),
            status: "success".to_string(),
            usage_source: "provider_reported".to_string(),
            input_tokens: Some(input),
            output_tokens: Some(output),
            total_tokens: Some(input + output),
            cache_read_tokens: Some(read),
            cache_write_tokens: Some(write),
            session_id: None,
            error_kind: None,
        }
    }

    /// 验证计费口径与原始口径同时给出且差距符合缓存折扣。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无；断言失败则测试不通过
    #[test]
    fn reports_raw_and_billable_input_side_by_side() {
        let records = vec![
            record(100_000, 500, 99_000, 0),
            record(100_000, 500, 99_000, 0),
        ];
        let summary = summarize(&records);
        assert_eq!(summary.input_tokens, 200_000);
        assert_eq!(summary.cache_read_tokens, 198_000);
        // 单条：1000 原价 + 99000 * 0.1 = 10900
        assert_eq!(summary.billable_input_tokens, 21_800);
        assert_eq!(summary.billable_total_tokens, 22_800);
    }

    /// 验证无缓存明细的历史记录两种口径一致。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无；断言失败则测试不通过
    #[test]
    fn keeps_legacy_records_unchanged() {
        let summary = summarize(&[record(1_000, 200, 0, 0)]);
        assert_eq!(summary.input_tokens, summary.billable_input_tokens);
        assert_eq!(summary.total_tokens, summary.billable_total_tokens);
    }
}
