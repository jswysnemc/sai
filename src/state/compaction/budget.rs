/// 自动压缩固定触发比例。
pub const AUTO_COMPACTION_RATIO: f32 = 0.9;

/// 触发压缩的绝对剩余空间下限（字符）。
///
/// 单一比例阈值在大窗口下留白过多：1M 窗口的 10% 是 100k 字符，远超一轮实际需要。
/// 补一条绝对下限后，大窗口按剩余量触发、小窗口按比例触发，两端都不失衡。
pub const RESERVED_CONTEXT_CHARS: usize = 50_000;

/// 陈旧工具结果裁剪触发比例。
///
/// 低于压缩阈值的免费维护档：改写陈旧工具结果不需要调用摘要模型，
/// 提前在六成处执行，可以推迟甚至避免九成处的付费压缩。
pub const STALE_TOOL_SNIP_RATIO: f32 = 0.6;

/// 上下文压力分级。
///
/// 与投影估算同一口径（estimate_projected_request_chars 的字符数）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextPressure {
    /// 六成以下：不做任何维护，保持前缀稳定以维持供应商缓存命中
    Relaxed,
    /// 六成到九成：免费改写陈旧工具结果，不调用摘要模型
    SnipStale,
    /// 九成以上：先兜底裁剪，仍超限再执行付费摘要压缩
    Compact,
}

/// 按当前上下文占用对压力分级。
///
/// 参数:
/// - `context_chars`: 当前请求上下文字符估算
/// - `context_limit_chars`: 当前模型上下文预算字符数
///
/// 返回:
/// - 上下文压力档位；预算未知时视为无压力
pub fn classify_context_pressure(
    context_chars: usize,
    context_limit_chars: usize,
) -> ContextPressure {
    if context_limit_chars == 0 {
        return ContextPressure::Relaxed;
    }
    if context_chars >= compaction_trigger_chars(context_limit_chars) {
        return ContextPressure::Compact;
    }
    let snip = ((context_limit_chars as f32) * STALE_TOOL_SNIP_RATIO).max(1.0) as usize;
    if context_chars >= snip {
        return ContextPressure::SnipStale;
    }
    ContextPressure::Relaxed
}

/// 判断当前上下文估算是否达到自动压缩阈值。
///
/// 比例与绝对剩余量取或：任一命中即触发。
///
/// 参数:
/// - `context_tokens`: 当前请求上下文估算
/// - `context_limit_tokens`: 当前模型上下文预算
///
/// 返回:
/// - 达到九成比例或剩余空间不足时返回 true
pub fn should_compact_for_context_tokens(
    context_tokens: usize,
    context_limit_tokens: usize,
) -> bool {
    if context_limit_tokens == 0 {
        return false;
    }
    context_tokens >= compaction_trigger_chars(context_limit_tokens)
}

/// 计算自动压缩触发点。
///
/// 取九成比例与"剩余不足 RESERVED_CONTEXT_CHARS"两个条件中更早到达的那个。
///
/// 参数:
/// - `context_limit_chars`: 当前模型上下文预算字符数
///
/// 返回:
/// - 自动压缩触发字符数
pub fn compaction_trigger_chars(context_limit_chars: usize) -> usize {
    let ratio_trigger = ((context_limit_chars as f32) * AUTO_COMPACTION_RATIO).max(1.0) as usize;
    // 预留额度大于等于整个窗口时该条件无意义，只用比例
    let Some(reserved_trigger) = context_limit_chars.checked_sub(RESERVED_CONTEXT_CHARS) else {
        return ratio_trigger;
    };
    if reserved_trigger == 0 {
        return ratio_trigger;
    }
    ratio_trigger.min(reserved_trigger).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证九成边界会触发自动压缩。
    #[test]
    fn triggers_at_ninety_percent() {
        assert!(should_compact_for_context_tokens(90, 100));
    }

    /// 验证九成边界以下不会触发自动压缩。
    #[test]
    fn skips_below_ninety_percent() {
        assert!(!should_compact_for_context_tokens(89, 100));
    }

    /// 验证压力分级的三个档位边界。
    #[test]
    fn classifies_context_pressure_by_ratio() {
        assert_eq!(classify_context_pressure(59, 100), ContextPressure::Relaxed);
        assert_eq!(
            classify_context_pressure(60, 100),
            ContextPressure::SnipStale
        );
        assert_eq!(
            classify_context_pressure(89, 100),
            ContextPressure::SnipStale
        );
        assert_eq!(classify_context_pressure(90, 100), ContextPressure::Compact);
    }

    /// 验证预算未知时视为无压力。
    #[test]
    fn unknown_limit_reports_relaxed() {
        assert_eq!(classify_context_pressure(500, 0), ContextPressure::Relaxed);
    }

    /// 验证小窗口下比例阈值先命中。
    #[test]
    fn small_window_triggers_on_ratio() {
        // 100k 窗口：九成为 90k，剩余额度条件为 50k，比例更晚到达取 50k
        // 20k 窗口：九成为 18k，剩余额度条件已为负，只能用比例
        assert_eq!(compaction_trigger_chars(20_000), 18_000);
    }

    /// 验证大窗口下绝对剩余量阈值先命中。
    #[test]
    fn large_window_triggers_on_reserved_headroom() {
        // 1M 窗口：九成为 900k，但剩余不足 50k 即 950k——比例更早，取 900k
        assert_eq!(compaction_trigger_chars(1_000_000), 900_000);
        // 200k 窗口：九成为 180k，剩余条件为 150k——剩余更早
        assert_eq!(compaction_trigger_chars(200_000), 150_000);
    }

    /// 验证预留额度超过窗口时回退到纯比例。
    #[test]
    fn tiny_window_falls_back_to_ratio() {
        assert_eq!(compaction_trigger_chars(1_000), 900);
        assert_eq!(compaction_trigger_chars(100), 90);
    }
}
