use super::compaction_schema::REPLAY_INSTRUCTION;
use super::Agent;
use crate::llm::ChatMessage;
use crate::state::request_projection::ProjectedRequest;

/// 复用会话前缀时为摘要输出预留的字符数。
///
/// 压缩在上下文占用九成时触发，回放整段前缀后剩下的余量本就不多。
/// 这里按实际生成体量预留，而不是按 summary_char_limit 的窗口比例：
/// 后者在九成占用下必然算出超窗，会让这条路径永远走不到。
///
/// 九节都必填、第 3 节还要带代码原文，产出比自由文本笔记长出一截，
/// 预留不足会让摘要生成到一半撞窗，那比退回独立请求糟得多。
const REPLAY_OUTPUT_RESERVE_CHARS: usize = 16_000;

/// 复用会话前缀的摘要请求。
pub(super) struct ReplaySummaryRequest {
    pub(super) messages: Vec<ChatMessage>,
    pub(super) definitions: Vec<crate::llm::ToolDefinition>,
}

impl Agent {
    /// 构造复用供应商前缀缓存的摘要请求。
    ///
    /// 常规做法是把历史渲染成文本塞进一条全新的用户消息，那样整个前缀
    /// 都是新的，摘要调用必然从零计费。这里改为原样回放本轮即将发出的
    /// 消息，只在末尾追加一条指令：请求于是成为会话请求的真前缀，
    /// 供应商的热缓存可以一路命中到指令为止。
    ///
    /// 参数:
    /// - `projection`: 本轮压缩决策所依据的请求投影
    ///
    /// 返回:
    /// - 可发送的摘要请求；不满足复用条件时返回 None
    pub(super) fn build_replay_summary_request(
        &self,
        projection: &ProjectedRequest,
    ) -> Option<ReplaySummaryRequest> {
        if !self.compaction_shares_session_route() {
            return None;
        }
        if !replay_fits_context(projection) {
            return None;
        }
        if projection.messages.is_empty() {
            return None;
        }
        Some(ReplaySummaryRequest {
            messages: replay_messages(projection.messages.clone()),
            // 工具定义必须与会话请求一致：少一个 schema 前缀就不同了，
            // 缓存从工具定义处就断开。指令里已禁止调用工具
            definitions: if self.tools_enabled {
                self.tool_visibility.definitions(&self.tools)
            } else {
                Vec::new()
            },
        })
    }

    /// 判断压缩模型是否与会话模型同源。
    ///
    /// 供应商与模型任一不同，缓存本就不通用，回放前缀只是白白多发一份历史。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 压缩请求与会话请求走同一条路由时为真
    fn compaction_shares_session_route(&self) -> bool {
        self.compaction_client.provider_id() == self.client.provider_id()
            && self.compaction_client.model() == self.client.model()
    }
}

/// 把本轮消息回放成摘要请求，指令追加在末尾。
///
/// 实际请求发出前会经 system_messages_first 规范化（前导 system 合并、
/// 中途 system 转 user）。这里必须走同一个函数：少这一步，回放出来的
/// 前缀与供应商缓存里的那份逐字不同，缓存一条也命中不了。
///
/// 参数:
/// - `messages`: 本轮尚未规范化的消息
///
/// 返回:
/// - 规范化后的消息序列，末尾为压缩指令
fn replay_messages(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    let mut replayed = super::message_context::system_messages_first(messages);
    replayed.push(ChatMessage::plain("user", REPLAY_INSTRUCTION.as_str()));
    replayed
}

/// 判断回放整段前缀后是否还放得下指令与摘要输出。
///
/// 参数:
/// - `projection`: 本轮请求投影
///
/// 返回:
/// - 余量足够时为真；预算未知时为假
fn replay_fits_context(projection: &ProjectedRequest) -> bool {
    let limit = projection.estimate.context_limit_chars;
    if limit == 0 {
        return false;
    }
    let required = projection
        .estimate
        .message_chars
        .saturating_add(REPLAY_INSTRUCTION.len())
        .saturating_add(REPLAY_OUTPUT_RESERVE_CHARS);
    required <= limit
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::request_projection::{ProjectionEstimate, ProjectionKind};

    /// 构造指定占用与预算的测试投影。
    ///
    /// 参数:
    /// - `message_chars`: 消息字符数
    /// - `context_limit_chars`: 上下文预算字符数
    ///
    /// 返回:
    /// - 请求投影
    fn projection(message_chars: usize, context_limit_chars: usize) -> ProjectedRequest {
        ProjectedRequest {
            kind: ProjectionKind::ProviderTurn,
            messages: vec![ChatMessage::plain("user", "hi")],
            tool_count: 0,
            estimate: ProjectionEstimate {
                message_chars,
                context_limit_chars,
                ..Default::default()
            },
            dynamic_sources: Vec::new(),
            provider_user_content: None,
            warnings: Vec::new(),
        }
    }

    /// 验证大窗口下九成占用仍留得出回放余量。
    #[test]
    fn large_window_at_trigger_still_fits() {
        assert!(replay_fits_context(&projection(900_000, 1_000_000)));
    }

    /// 验证小窗口下九成占用放不下摘要输出。
    ///
    /// 这不是缺陷而是设计边界：窗口本身就没有余量再走一次全量回放，
    /// 此时应当回退到裁剪过的独立摘要请求。
    #[test]
    fn small_window_at_trigger_does_not_fit() {
        assert!(!replay_fits_context(&projection(90_000, 100_000)));
    }

    /// 验证预算未知时不启用回放。
    #[test]
    fn unknown_budget_disables_replay() {
        assert!(!replay_fits_context(&projection(1_000, 0)));
    }

    /// 验证回放结果是实际请求的逐字前缀。
    ///
    /// 这是整条优化成立的唯一前提：回放与实际请求走同一个规范化函数，
    /// 除末尾指令外每条消息都必须相同。任何一处不同都会让缓存整段失效，
    /// 而且不会报错，只会静默地按全新前缀计费。
    #[test]
    fn replayed_messages_are_a_verbatim_prefix_of_the_real_request() {
        let raw = vec![
            ChatMessage::system("base"),
            ChatMessage::system("epoch"),
            ChatMessage::plain("user", "task"),
            ChatMessage::plain("assistant", "working"),
        ];

        let sent = super::super::message_context::system_messages_first(raw.clone());
        let replayed = replay_messages(raw);

        assert_eq!(replayed.len(), sent.len() + 1);
        for (index, message) in sent.iter().enumerate() {
            assert_eq!(replayed[index].role, message.role);
            assert_eq!(
                format!("{:?}", replayed[index].content),
                format!("{:?}", message.content)
            );
        }
        assert_eq!(replayed[sent.len()].role, "user");
    }

    /// 验证回放同样合并了前导系统消息。
    ///
    /// 漏掉规范化时这里会是两条 system，与实际发出的一条对不上。
    #[test]
    fn replay_merges_leading_system_messages() {
        let replayed = replay_messages(vec![
            ChatMessage::system("base"),
            ChatMessage::system("epoch"),
            ChatMessage::plain("user", "task"),
        ]);

        assert_eq!(
            replayed
                .iter()
                .filter(|message| message.role == "system")
                .count(),
            1
        );
    }
}
