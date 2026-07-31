use crate::llm::{ChatMessage, ChatResult, Usage};

/// 一次用户轮次的执行结果。
///
/// 一个轮次内可能发生多次模型调用（每轮工具调用各一次），两种用量口径必须分开：
/// `result.usage` 是最后一次调用的用量，其 `prompt_tokens` 代表当前上下文占用；
/// `turn_usage` 是全部调用的累计，代表本轮实际消耗，用于计费统计。
/// 混用会让上下文进度条虚高，或让用量统计漏掉工具轮次。
pub struct TurnExecution {
    /// 最后一次模型调用的结果
    pub result: ChatResult,
    /// 本轮全部模型调用的用量累计
    pub turn_usage: Option<Usage>,
}

/// 轮次内模型调用的用量累加器。
#[derive(Default)]
pub struct TurnUsageAccumulator {
    total: Usage,
    reported: bool,
}

/// 【Agent】【工具子轮】构造需要回传给模型的 assistant 工具调用消息。
///
/// 参数:
/// - `result`: 当前模型子轮结果
///
/// 返回:
/// - 包含正文、工具调用和思考内容的 assistant 消息
pub fn assistant_tool_message(result: &ChatResult) -> ChatMessage {
    ChatMessage::assistant(result.content.clone(), Some(result.tool_calls.clone()))
        .with_reasoning(result.reasoning.clone())
}

impl TurnUsageAccumulator {
    /// 累加一次模型调用的用量。
    ///
    /// 参数:
    /// - `usage`: 本次调用的用量，未上报时为 None
    ///
    /// 返回:
    /// - 无；仅在有上报时更新累计
    pub fn add(&mut self, usage: Option<&Usage>) {
        let Some(usage) = usage else {
            return;
        };
        self.total.accumulate(usage);
        self.reported = true;
    }

    /// 收尾并取出累计用量。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 累计用量；本轮没有任何上报时为 None
    pub fn finish(self) -> Option<Usage> {
        self.reported.then_some(self.total)
    }

    /// 用累计结果组装轮次执行结果。
    ///
    /// 参数:
    /// - `result`: 最后一次模型调用的结果
    ///
    /// 返回:
    /// - 携带两种用量口径的执行结果
    pub fn into_execution(self, result: ChatResult) -> TurnExecution {
        TurnExecution {
            turn_usage: self.finish(),
            result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(prompt: u64, completion: u64, cache_read: u64) -> Usage {
        Usage {
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: prompt + completion,
            cache_read_tokens: cache_read,
            cache_write_tokens: 0,
        }
    }

    /// 验证多轮调用的用量按字段累加。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无；断言失败则测试不通过
    #[test]
    fn accumulates_every_round() {
        let mut acc = TurnUsageAccumulator::default();
        acc.add(Some(&usage(1_000, 100, 800)));
        acc.add(Some(&usage(2_000, 200, 1_800)));
        acc.add(None);
        let total = acc.finish().unwrap();
        assert_eq!(total.prompt_tokens, 3_000);
        assert_eq!(total.completion_tokens, 300);
        assert_eq!(total.total_tokens, 3_300);
        assert_eq!(total.cache_read_tokens, 2_600);
    }

    /// 验证全程无上报时不产生累计用量。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无；断言失败则测试不通过
    #[test]
    fn yields_none_without_any_report() {
        let mut acc = TurnUsageAccumulator::default();
        acc.add(None);
        assert!(acc.finish().is_none());
    }

    /// 【Agent】【工具子轮】验证 assistant 工具消息保留 DeepSeek 思考内容。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn assistant_tool_message_preserves_reasoning() {
        let result = ChatResult {
            content: "准备调用工具".to_string(),
            reasoning: Some("先查询日期".to_string()),
            usage: None,
            tool_calls: Vec::new(),
            duration_ms: 0,
        };

        let message = assistant_tool_message(&result);

        assert_eq!(message.reasoning_content.as_deref(), Some("先查询日期"));
    }
}
