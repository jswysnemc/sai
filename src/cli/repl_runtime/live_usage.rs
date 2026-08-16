use crate::llm::Usage;

/// 当前轮次进行中的实时用量。
///
/// Agent 每完成一次 provider 请求就发出 `ContextUpdated`，这里按轮累计，
/// 让底栏在轮次结束前就能反映真实的上下文占用与缓存命中，
/// 而不是停留在上一轮结束时的读数。
#[derive(Debug, Clone, Default)]
pub(super) struct LiveTurnUsage {
    /// 最近一次请求 provider 实报的 prompt tokens，即当前上下文占用
    context_prompt_tokens: usize,
    cache_read_tokens: u64,
    prompt_tokens: u64,
}

impl LiveTurnUsage {
    /// 累计一次已完成的 provider 请求。
    ///
    /// 参数:
    /// - `usage`: 本次请求 provider 上报的用量
    ///
    /// 返回:
    /// - 无
    pub(super) fn record(&mut self, usage: &Usage) {
        // 1. 上下文占用取最近一次读数，不累加：它表示当前请求体有多大
        if usage.prompt_tokens > 0 {
            self.context_prompt_tokens = usage.prompt_tokens as usize;
        }
        // 2. 缓存命中率按轮累计，与轮次结束摘要口径一致
        self.prompt_tokens = self.prompt_tokens.saturating_add(usage.prompt_tokens);
        self.cache_read_tokens = self
            .cache_read_tokens
            .saturating_add(usage.cache_read_tokens.min(usage.prompt_tokens));
    }

    /// 返回当前上下文占用 token 数。
    ///
    /// 返回:
    /// - 最近一次请求的 prompt tokens；尚无读数时返回空
    pub(super) fn context_prompt_tokens(&self) -> Option<usize> {
        (self.context_prompt_tokens > 0).then_some(self.context_prompt_tokens)
    }

    /// 返回本轮累计缓存命中率。
    ///
    /// 返回:
    /// - 命中率；尚无读数时返回空
    pub(super) fn cache_hit_ratio(&self) -> Option<f32> {
        (self.prompt_tokens > 0).then(|| self.cache_read_tokens as f32 / self.prompt_tokens as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造测试用量。
    ///
    /// 参数:
    /// - `prompt`: prompt tokens
    /// - `cache_read`: 命中缓存的 tokens
    ///
    /// 返回:
    /// - 用量
    fn usage(prompt: u64, cache_read: u64) -> Usage {
        Usage {
            prompt_tokens: prompt,
            completion_tokens: 0,
            total_tokens: prompt,
            cache_read_tokens: cache_read,
            cache_write_tokens: 0,
        }
    }

    /// 【TUI】【实时用量】验证上下文取最近读数而非累加。
    #[test]
    fn context_tokens_track_latest_request() {
        let mut live = LiveTurnUsage::default();
        live.record(&usage(1_000, 0));
        live.record(&usage(1_500, 1_000));

        assert_eq!(live.context_prompt_tokens(), Some(1_500));
    }

    /// 【TUI】【实时用量】验证缓存命中率按轮累计。
    #[test]
    fn cache_ratio_accumulates_across_requests() {
        let mut live = LiveTurnUsage::default();
        live.record(&usage(1_000, 0));
        live.record(&usage(1_000, 1_000));

        assert_eq!(live.cache_hit_ratio(), Some(0.5));
    }

    /// 【TUI】【实时用量】验证无读数时不产出比率。
    #[test]
    fn empty_usage_reports_nothing() {
        let live = LiveTurnUsage::default();

        assert_eq!(live.context_prompt_tokens(), None);
        assert_eq!(live.cache_hit_ratio(), None);
    }
}
