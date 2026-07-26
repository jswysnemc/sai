/// 自动压缩固定触发比例。
pub const AUTO_COMPACTION_RATIO: f32 = 0.9;

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

/// 判断当前上下文 token 估算是否达到自动压缩阈值。
///
/// 参数:
/// - `context_tokens`: 当前请求上下文 token 估算
/// - `context_limit_tokens`: 当前模型上下文窗口 token 数
///
/// 返回:
/// - 达到固定九成阈值时返回 true
pub fn should_compact_for_context_tokens(
    context_tokens: usize,
    context_limit_tokens: usize,
) -> bool {
    if context_limit_tokens == 0 {
        return false;
    }
    context_tokens >= compaction_trigger_chars(context_limit_tokens)
}

/// 计算固定九成自动压缩阈值。
///
/// 参数:
/// - `context_limit_chars`: 当前模型上下文预算字符数
///
/// 返回:
/// - 自动压缩触发字符数
pub fn compaction_trigger_chars(context_limit_chars: usize) -> usize {
    ((context_limit_chars as f32) * AUTO_COMPACTION_RATIO).max(1.0) as usize
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
}
