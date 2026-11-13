use crate::llm::Usage;
use std::time::Instant;

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
    started: Option<Instant>,
    stream_text: String,
    completed_tokens: usize,
    stream_tokens: usize,
    generation_seconds: f64,
    last_rate: Option<f64>,
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
        if let Some(started) = self.started.take() {
            self.generation_seconds += started.elapsed().as_secs_f64();
            self.completed_tokens += usage.completion_tokens as usize;
            self.stream_text.clear();
            self.stream_tokens = 0;
            self.last_rate = self.rate();
        }
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

    /// 【终端】【流速采样】收集生成文本；参数为流分片，返回无值，缓冲区保持有界
    pub(super) fn chunk(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.started.get_or_insert_with(Instant::now);
        self.stream_text.push_str(text);
        if self.stream_text.len() > 65_536 {
            self.stream_tokens += crate::token_estimate::estimate_tokens(&self.stream_text);
            self.stream_text.clear();
        }
    }

    /// 【终端】【生成速率】返回生成阶段的平均 tokens 每秒速率，流式期间使用分词估算
    pub(super) fn rate(&self) -> Option<f64> {
        let seconds = self.generation_seconds
            + self
                .started
                .map_or(0.0, |start| start.elapsed().as_secs_f64());
        let tokens = self.completed_tokens
            + self.stream_tokens
            + crate::token_estimate::estimate_tokens(&self.stream_text);
        (seconds >= 0.1 && tokens > 0)
            .then(|| tokens as f64 / seconds)
            .or(self.last_rate)
    }

    /// 【终端】【重试采样】撤销当前尝试的未结算文本，无参数和返回值
    pub(super) fn reset_response(&mut self) {
        self.started = None;
        self.stream_text.clear();
        self.stream_tokens = 0;
    }

    /// 【终端】【采样收尾】冻结最终速率，避免空闲时间使显示值持续下降；无参数和返回值
    pub(super) fn finish(&mut self) {
        self.last_rate = self.rate();
        if let Some(start) = self.started.take() {
            self.generation_seconds += start.elapsed().as_secs_f64();
        }
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

#[cfg(test)]
mod rate_tests {
    use super::*;
    /// 【终端】【长回复速率】超过缓冲区的估算不能与提供方完整用量重复累计
    #[test]
    fn long_response_replaces_estimates_with_provider_usage() {
        let mut live = LiveTurnUsage::default();
        live.chunk(&"a ".repeat(40_000));
        live.started = Some(Instant::now() - std::time::Duration::from_secs(1));
        live.record(&Usage {
            prompt_tokens: 20,
            completion_tokens: 100,
            total_tokens: 120,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        });
        assert_eq!(live.completed_tokens, 100);
        assert_eq!(live.stream_tokens, 0);
        assert!((live.rate().unwrap() - 100.0).abs() < 5.0);
    }
    /// 【终端】【重试撤销】失败尝试的文字不进入后续生成速率
    #[test]
    fn response_reset_discards_only_current_attempt() {
        let mut live = LiveTurnUsage::default();
        live.chunk("discarded content");
        live.reset_response();
        assert!(live.started.is_none());
        assert!(live.stream_text.is_empty());
        assert_eq!(live.stream_tokens, 0);
    }
}
