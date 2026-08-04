use super::extraction_prompt::{
    build_extraction_input, parse_extraction_output, EXTRACTION_SYSTEM_PROMPT,
};
use crate::llm::{ChatMessage, OpenAiCompatibleClient};
use crate::memory::model::MemoryCandidate;
use anyhow::Result;
use std::time::Duration;

/// 抽取请求的超时时间。
///
/// 抽取在轮次结束后异步执行，超时只丢掉这一轮的记忆，不影响用户可见的答复。
const EXTRACTION_TIMEOUT: Duration = Duration::from_secs(45);

/// 提交给抽取模型的对话原文上限。
///
/// 超长轮次的信息密度并不更高，截断可以显著压低抽取成本。
const MAX_INPUT_CHARS: usize = 12_000;

/// 从一轮对话中抽取值得长期记住的候选。
///
/// 旧实现用字符串裁剪冒充提炼：去填充词、找结论句、截断长度，
/// 产出的是「时间：任务 → 结果」的对话流水账。这里让模型判断
/// 哪些信息在未来无关的对话中仍然有用。
///
/// 参数:
/// - `client`: 抽取使用的 LLM 客户端
/// - `user_message`: 用户原文
/// - `assistant_message`: 助手回复原文
/// - `workspace`: 当前工作区路径；无工作区时为 None
///
/// 返回:
/// - 归一化后的候选；模型判定无内容可记时为空
pub async fn extract_candidates(
    client: &OpenAiCompatibleClient,
    user_message: &str,
    assistant_message: &str,
    workspace: Option<&str>,
) -> Result<Vec<MemoryCandidate>> {
    let input = build_extraction_input(
        &clip(user_message),
        &clip(assistant_message),
        workspace,
    );
    let messages = vec![
        ChatMessage::system(EXTRACTION_SYSTEM_PROMPT),
        ChatMessage::plain("user", input),
    ];
    let result = tokio::time::timeout(
        EXTRACTION_TIMEOUT,
        client.chat_stream(messages, Vec::new(), |_| Ok(())),
    )
    .await
    .map_err(|_| anyhow::anyhow!("memory extraction timed out"))??;
    parse_extraction_output(&result.content, workspace)
}

/// 截断过长的对话原文。
///
/// 参数:
/// - `text`: 原文
///
/// 返回:
/// - 不超过上限的文本
fn clip(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= MAX_INPUT_CHARS {
        return trimmed.to_string();
    }
    trimmed.chars().take(MAX_INPUT_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证短文本不被截断。
    #[test]
    fn short_text_passes_through_unchanged() {
        assert_eq!(clip("  用户的问题  "), "用户的问题");
    }

    /// 验证超长文本被截断到上限。
    #[test]
    fn long_text_is_clipped_to_the_limit() {
        let long = "字".repeat(MAX_INPUT_CHARS + 500);
        assert_eq!(clip(&long).chars().count(), MAX_INPUT_CHARS);
    }
}
