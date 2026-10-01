use crate::llm::{ChatContent, ChatMessage};
use crate::state::compaction::estimate_chat_messages_tokens;
use crate::state::StateStore;
use anyhow::Result;
use rusqlite::params;
use std::collections::HashMap;

/// 【上下文预算】【结果替换】同一请求替换后的消息与 token 占用。
pub(crate) struct RefreshedToolContext {
    pub messages: Vec<ChatMessage>,
    pub token_count: usize,
}

impl StateStore {
    /// 【上下文预算】【结果替换】只应用当前可见工具的替换，并以 token 差值更新占用。
    /// @param messages 为当前请求；token_count 为其已有占用，允许来自供应商 usage
    /// @returns 替换后的请求与同单位占用，不计入其它分支的字符节省量
    pub(crate) fn refresh_tool_context(
        &self,
        messages: &[ChatMessage],
        token_count: usize,
    ) -> Result<RefreshedToolContext> {
        let call_ids = messages
            .iter()
            .filter(|message| message.role == "tool")
            .filter_map(|message| message.tool_call_id.as_deref())
            .collect::<Vec<_>>();
        let replacements = {
            let conn = self.conv_db.conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT provider_call_id, replacement FROM tool_output_replacements
                 WHERE session_id = ?1 AND provider_call_id IN (SELECT value FROM json_each(?2))",
            )?;
            let rows = stmt.query_map(
                params![self.session_id, serde_json::to_string(&call_ids)?],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )?;
            rows.collect::<std::result::Result<HashMap<_, _>, _>>()?
        };
        let mut messages = messages.to_vec();
        let mut removed_tokens = 0usize;
        let mut added_tokens = 0usize;
        for message in messages.iter_mut().filter(|message| message.role == "tool") {
            let Some(replacement) = message
                .tool_call_id
                .as_ref()
                .and_then(|id| replacements.get(id))
            else {
                continue;
            };
            if matches!(message.content.as_ref(), Some(ChatContent::Text(text)) if text == replacement)
            {
                continue;
            }
            removed_tokens = removed_tokens
                .saturating_add(estimate_chat_messages_tokens(std::slice::from_ref(message)));
            message.content = Some(ChatContent::Text(replacement.clone()));
            added_tokens = added_tokens
                .saturating_add(estimate_chat_messages_tokens(std::slice::from_ref(message)));
        }
        Ok(RefreshedToolContext {
            messages,
            token_count: token_count
                .saturating_sub(removed_tokens)
                .saturating_add(added_tokens),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::Usage;
    use crate::paths::SaiPaths;

    /// 【上下文预算】【单位回归】字符减少很多仍可能超过 token 窗口；无参数或返回值。
    #[test]
    fn replacement_uses_token_savings_from_visible_results_only() {
        let directory = tempfile::tempdir().unwrap();
        let state = StateStore::new(&SaiPaths::for_tests(directory.path())).unwrap();
        let original = "hello world ".repeat(3500);
        state
            .save_clipped_tool_output_replacement("visible", &original, "elided")
            .unwrap();
        state
            .save_clipped_tool_output_replacement("other-branch", &"unused".repeat(10000), "elided")
            .unwrap();
        let messages = vec![
            ChatMessage::system("stable"),
            ChatMessage::tool("visible", &original),
        ];
        let refreshed = state.refresh_tool_context(&messages, 110_000).unwrap();
        assert!(refreshed.token_count > 100_000);
        assert_eq!(
            refreshed.token_count,
            110_000 - crate::token_estimate::estimate_tokens(&original)
                + crate::token_estimate::estimate_tokens("elided")
        );
        assert!(
            matches!(&refreshed.messages[0].content, Some(ChatContent::Text(text)) if text == "stable")
        );
        assert!(
            matches!(&refreshed.messages[1].content, Some(ChatContent::Text(text)) if text == "elided")
        );
        assert_eq!(
            state
                .refresh_tool_context(&[ChatMessage::plain("user", "unchanged")], 110_000)
                .unwrap()
                .token_count,
            110_000
        );
    }

    /// 【上下文预算】【尾部回归】提醒不能遮住尚未计费的工具结果；无参数或返回值。
    #[test]
    fn unsent_tail_includes_tools_images_and_reminders() {
        let tail = vec![
            ChatMessage::tool("call", "large result".repeat(100)),
            ChatMessage::plain("user", "attachment"),
            ChatMessage::plain("user", "reminder"),
        ];
        let mut messages = vec![ChatMessage::plain("assistant", "already billed")];
        messages.extend(tail.clone());
        let usage = Usage {
            prompt_tokens: 100,
            completion_tokens: 20,
            ..Usage::default()
        };
        assert_eq!(
            crate::state::occupancy_tokens(&messages, Some(&usage)),
            120 + estimate_chat_messages_tokens(&tail)
        );
    }
}
