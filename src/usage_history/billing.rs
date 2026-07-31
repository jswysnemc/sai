use super::record::UsageRecord;

/// 缓存令牌的计价系数。
///
/// 供应商对提示词缓存单独定价：命中读取远低于标准输入价，写入则略高于标准价。
/// 本地若把三段输入量等同看待，统计结果会与账单出现数量级偏差。
#[derive(Debug, Clone, Copy)]
pub struct CacheRates {
    /// 命中缓存读取相对标准输入价的比例
    pub read: f64,
    /// 写入缓存相对标准输入价的比例
    pub write: f64,
}

/// Anthropic 口径：读取一折，写入一点二五倍。
pub const ANTHROPIC_RATES: CacheRates = CacheRates {
    read: 0.1,
    write: 1.25,
};

/// OpenAI 口径：命中读取半价，不单独计写入。
pub const OPENAI_RATES: CacheRates = CacheRates {
    read: 0.5,
    write: 1.0,
};

/// DeepSeek V4 Flash 口径：缓存命中输入价为未命中的 2%。
pub const DEEPSEEK_V4_FLASH_RATES: CacheRates = CacheRates {
    read: 0.02,
    write: 1.0,
};

/// DeepSeek V4 Pro 口径：0.025 元相对 3 元未命中输入价。
pub const DEEPSEEK_V4_PRO_RATES: CacheRates = CacheRates {
    read: 0.008_333_333_333_333_333,
    write: 1.0,
};

/// 按供应商标识推断缓存计价系数。
///
/// 参数:
/// - `provider_id`: 记录中的供应商标识
/// - `model`: 记录中的模型名
///
/// 返回:
/// - 匹配到的计价系数；无法判断时回落到 Anthropic 口径，
///   因为该口径差异最大，用它估算不会低估与账单的偏离程度
pub fn rates_for(provider_id: &str, model: &str) -> CacheRates {
    let haystack = format!("{provider_id} {model}").to_ascii_lowercase();
    if haystack.contains("deepseek-v4-flash") {
        return DEEPSEEK_V4_FLASH_RATES;
    }
    if haystack.contains("deepseek-v4-pro") {
        return DEEPSEEK_V4_PRO_RATES;
    }
    if haystack.contains("gpt") || haystack.contains("openai") || haystack.contains("o1") {
        return OPENAI_RATES;
    }
    ANTHROPIC_RATES
}

/// 折算单条记录的等效计费输入令牌。
///
/// 参数:
/// - `record`: 用量记录
///
/// 返回:
/// - 未命中缓存部分按原价、缓存读写按各自系数折算后的输入令牌数
pub fn billable_input_tokens(record: &UsageRecord) -> u64 {
    let input = record.input_tokens.unwrap_or(0);
    let cache_read = record.cache_read_tokens.unwrap_or(0);
    let cache_write = record.cache_write_tokens.unwrap_or(0);
    // 1. 缓存明细缺失时无从折算，按原始输入量返回
    if cache_read == 0 && cache_write == 0 {
        return input;
    }
    // 2. 扣除缓存部分得到按标准价计费的输入量
    let uncached = input.saturating_sub(cache_read).saturating_sub(cache_write);
    let rates = rates_for(&record.provider_id, &record.model);
    // 3. 各段按系数折算后求和
    let billable =
        uncached as f64 + cache_read as f64 * rates.read + cache_write as f64 * rates.write;
    billable.round().max(0.0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_with(provider: &str, model: &str, input: u64, read: u64, write: u64) -> UsageRecord {
        UsageRecord {
            id: "u1".to_string(),
            created_at: 0,
            completed_at: 0,
            duration_ms: 0,
            source: "chat".to_string(),
            operation: "turn".to_string(),
            provider_id: provider.to_string(),
            provider_name: provider.to_string(),
            model: model.to_string(),
            status: "success".to_string(),
            usage_source: "provider_reported".to_string(),
            input_tokens: Some(input),
            output_tokens: Some(0),
            total_tokens: Some(input),
            cache_read_tokens: Some(read),
            cache_write_tokens: Some(write),
            session_id: None,
            error_kind: None,
        }
    }

    /// 验证高缓存命中时折算量远低于原始输入量。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无；断言失败则测试不通过
    #[test]
    fn discounts_cache_heavy_input() {
        // 输入 10 万，其中 9.5 万来自缓存读取
        let record = record_with("anthropic", "claude-opus", 100_000, 95_000, 0);
        // 5000 原价 + 95000 * 0.1 = 14500
        assert_eq!(billable_input_tokens(&record), 14_500);
    }

    /// 验证缓存明细缺失时按原始输入量返回。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无；断言失败则测试不通过
    #[test]
    fn falls_back_to_raw_input_without_cache_detail() {
        let record = record_with("legacy", "model-a", 1_200, 0, 0);
        assert_eq!(billable_input_tokens(&record), 1_200);
    }

    /// 验证 OpenAI 系模型走半价系数。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无；断言失败则测试不通过
    #[test]
    fn uses_openai_rates_for_gpt_models() {
        let record = record_with("openai", "gpt-4o", 1_000, 800, 0);
        // 200 原价 + 800 * 0.5 = 600
        assert_eq!(billable_input_tokens(&record), 600);
    }

    /// 【用量统计】【DeepSeek】验证 V4 Flash 使用官方缓存输入比例。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn uses_deepseek_v4_flash_cache_rate() {
        let record = record_with("deepseek", "deepseek-v4-flash", 1_000, 800, 0);

        // 200 未命中 + 800 * 0.02 = 216
        assert_eq!(billable_input_tokens(&record), 216);
    }

    /// 【用量统计】【DeepSeek】验证 V4 Pro 使用官方缓存输入比例。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn uses_deepseek_v4_pro_cache_rate() {
        let record = record_with("deepseek", "deepseek-v4-pro", 1_000, 800, 0);

        // 200 未命中 + 800 * (0.025 / 3) ≈ 207
        assert_eq!(billable_input_tokens(&record), 207);
    }
}
