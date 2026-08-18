/// 自动压缩默认触发比例。
pub const AUTO_COMPACTION_RATIO: f32 = 0.9;

/// 默认压缩预留 token。大窗口按「至少留这么多」触发，小窗口仍走比例。
pub const RESERVED_CONTEXT_CHARS: usize = 50_000;

/// 允许配置的压缩比例下限。
pub const MIN_COMPACTION_RATIO: f32 = 0.50;

/// 允许配置的压缩比例上限。
pub const MAX_COMPACTION_RATIO: f32 = 0.99;

/// 陈旧工具结果裁剪触发比例。
///
/// 低于压缩阈值的免费维护档：改写陈旧工具结果不需要调用摘要模型，
/// 提前在六成处执行，可以推迟甚至避免九成处的付费压缩。
pub const STALE_TOOL_SNIP_RATIO: f32 = 0.6;

/// 会话级自动压缩触发策略。
///
/// 比例决定小窗口何时压缩；预留决定大窗口至少留多少空位。
/// 两者取更晚到达的那个，避免小窗口被固定 40k/50k 预留过早压缩。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompactionBudgetPolicy {
    /// 占用达到窗口的该比例时触发，范围 0.50–0.99
    pub ratio: f32,
    /// 至少留出的 token；0 表示只按比例
    pub reserve_tokens: usize,
}

impl Default for CompactionBudgetPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl CompactionBudgetPolicy {
    /// 内置默认：90% 或预留 50k，按窗口取更晚到达的条件。
    pub const DEFAULT: Self = Self {
        ratio: AUTO_COMPACTION_RATIO,
        reserve_tokens: RESERVED_CONTEXT_CHARS,
    };

    /// 从会话上下文配置构造并夹紧到合法范围。
    ///
    /// 参数:
    /// - `ratio`: 配置的压缩比例
    /// - `reserve_tokens`: 配置的预留 token
    ///
    /// 返回:
    /// - 夹紧后的策略
    pub fn from_context(ratio: f32, reserve_tokens: usize) -> Self {
        Self {
            ratio: clamp_compaction_ratio(ratio),
            reserve_tokens,
        }
    }

    /// 计算自动压缩触发点。
    ///
    /// 参数:
    /// - `context_limit_chars`: 当前模型上下文预算
    ///
    /// 返回:
    /// - 自动压缩触发 token 数
    pub fn trigger_chars(self, context_limit_chars: usize) -> usize {
        if context_limit_chars == 0 {
            return 0;
        }
        let ratio_trigger = ((context_limit_chars as f32) * self.ratio).max(1.0) as usize;
        if self.reserve_tokens == 0 || self.reserve_tokens >= context_limit_chars {
            return ratio_trigger.max(1);
        }
        let reserved_trigger = context_limit_chars - self.reserve_tokens;
        // 大窗口预留更晚到达，按剩余量压；小窗口比例更晚到达，按比例压
        ratio_trigger.max(reserved_trigger).max(1)
    }
}

/// 把压缩比例夹紧到合法区间。
///
/// 参数:
/// - `ratio`: 原始比例
///
/// 返回:
/// - 0.50–0.99 之间的比例
pub fn clamp_compaction_ratio(ratio: f32) -> f32 {
    if !ratio.is_finite() {
        return AUTO_COMPACTION_RATIO;
    }
    ratio.clamp(MIN_COMPACTION_RATIO, MAX_COMPACTION_RATIO)
}

/// 上下文压力分级。
///
/// 与投影估算同一口径（estimate_projected_request_chars 的字符数）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextPressure {
    /// 六成以下：不做任何维护，保持前缀稳定以维持供应商缓存命中
    Relaxed,
    /// 六成到压缩阈值：免费改写陈旧工具结果，不调用摘要模型
    SnipStale,
    /// 达到压缩阈值：先兜底裁剪，仍超限再执行付费摘要压缩
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
#[cfg(test)]
pub fn classify_context_pressure(
    context_chars: usize,
    context_limit_chars: usize,
) -> ContextPressure {
    classify_context_pressure_with(
        context_chars,
        context_limit_chars,
        CompactionBudgetPolicy::DEFAULT,
    )
}

/// 按指定策略对上下文压力分级。
///
/// 参数:
/// - `context_chars`: 当前请求上下文字符估算
/// - `context_limit_chars`: 当前模型上下文预算字符数
/// - `policy`: 会话级压缩策略
///
/// 返回:
/// - 上下文压力档位；预算未知时视为无压力
pub fn classify_context_pressure_with(
    context_chars: usize,
    context_limit_chars: usize,
    policy: CompactionBudgetPolicy,
) -> ContextPressure {
    if context_limit_chars == 0 {
        return ContextPressure::Relaxed;
    }
    if context_chars >= policy.trigger_chars(context_limit_chars) {
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
/// 参数:
/// - `context_tokens`: 当前请求上下文估算
/// - `context_limit_tokens`: 当前模型上下文预算
///
/// 返回:
/// - 达到默认策略阈值时返回 true
#[cfg(test)]
pub fn should_compact_for_context_tokens(
    context_tokens: usize,
    context_limit_tokens: usize,
) -> bool {
    should_compact_for_context_tokens_with(
        context_tokens,
        context_limit_tokens,
        CompactionBudgetPolicy::DEFAULT,
    )
}

/// 按指定策略判断是否达到自动压缩阈值。
///
/// 参数:
/// - `context_tokens`: 当前请求上下文估算
/// - `context_limit_tokens`: 当前模型上下文预算
/// - `policy`: 会话级压缩策略
///
/// 返回:
/// - 达到阈值时返回 true
pub fn should_compact_for_context_tokens_with(
    context_tokens: usize,
    context_limit_tokens: usize,
    policy: CompactionBudgetPolicy,
) -> bool {
    if context_limit_tokens == 0 {
        return false;
    }
    context_tokens >= policy.trigger_chars(context_limit_tokens)
}

/// 计算默认策略下的自动压缩触发点。
///
/// 参数:
/// - `context_limit_chars`: 当前模型上下文预算字符数
///
/// 返回:
/// - 自动压缩触发字符数
#[cfg(test)]
pub fn compaction_trigger_chars(context_limit_chars: usize) -> usize {
    CompactionBudgetPolicy::DEFAULT.trigger_chars(context_limit_chars)
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

    /// 验证小窗口下比例阈值先命中，不被固定预留拖早。
    #[test]
    fn small_window_triggers_on_ratio() {
        assert_eq!(compaction_trigger_chars(20_000), 18_000);
        assert_eq!(compaction_trigger_chars(80_000), 72_000);
        assert_eq!(compaction_trigger_chars(200_000), 180_000);
    }

    /// 验证大窗口下按预留空位触发，比九成更晚。
    #[test]
    fn large_window_triggers_on_reserved_headroom() {
        assert_eq!(compaction_trigger_chars(1_000_000), 950_000);
    }

    /// 验证预留额度超过窗口或为 0 时回退到纯比例。
    #[test]
    fn tiny_window_falls_back_to_ratio() {
        assert_eq!(compaction_trigger_chars(1_000), 900);
        assert_eq!(compaction_trigger_chars(100), 90);
        assert_eq!(
            CompactionBudgetPolicy::from_context(0.9, 0).trigger_chars(200_000),
            180_000
        );
    }

    /// 验证会话配置可以收紧预留，让中等窗口更晚压缩。
    #[test]
    fn session_policy_can_shrink_reserve() {
        let policy = CompactionBudgetPolicy::from_context(0.9, 8_000);
        assert_eq!(policy.trigger_chars(32_000), 28_800);
        assert_eq!(policy.trigger_chars(80_000), 72_000);
    }
}
