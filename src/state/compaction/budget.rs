/// 自动压缩固定触发比例。
pub const AUTO_COMPACTION_RATIO: f32 = 0.9;

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
    context_tokens >= trigger_tokens(context_limit_tokens)
}

/// 计算固定九成自动压缩阈值。
///
/// 参数:
/// - `context_limit_tokens`: 当前模型上下文窗口 token 数
///
/// 返回:
/// - 自动压缩触发 token 数
fn trigger_tokens(context_limit_tokens: usize) -> usize {
    ((context_limit_tokens as f32) * AUTO_COMPACTION_RATIO).max(1.0) as usize
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
}
