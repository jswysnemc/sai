use crate::llm::{ChatContent, ChatMessage, ChatResult, ProviderAssistantContent};
use crate::state::{ConversationDb, StateStore};
use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 【助手历史】【消息定位】区分工具子轮、间隙回复和最终回复，避免标识冲突。
pub(crate) enum AssistantMessageKey<'a> {
    Tool(&'a str),
    Intermediate(&'a str),
    Final,
}

impl AssistantMessageKey<'_> {
    /// 【助手历史】【消息定位】生成数据库键；无参数，返回有命名空间的消息标识。
    fn encoded(self) -> String {
        match self {
            Self::Tool(id) => format!("tool:{id}"),
            Self::Intermediate(id) => format!("message:{id}"),
            Self::Final => "final".into(),
        }
    }
}

/// 【助手历史】【消息原文】展示字段与供应商专用内容分别保存。
#[derive(Serialize, Deserialize)]
struct StoredAssistantMessage {
    content: String,
    reasoning: Option<String>,
    provider_content: Option<ProviderAssistantContent>,
}

/// 【助手历史】【轮次读取】一次读取同一轮次的全部原文，供投影复用。
#[derive(Default)]
pub(super) struct SavedAssistantMessages(HashMap<String, StoredAssistantMessage>);

impl SavedAssistantMessages {
    /// 【助手历史】【消息重放】恢复原文；参数为消息键及旧版回退消息，返回完整消息。
    pub(super) fn restore(
        &self,
        key: AssistantMessageKey<'_>,
        mut fallback: ChatMessage,
    ) -> ChatMessage {
        if let Some(saved) = self.0.get(&key.encoded()) {
            fallback.content = Some(ChatContent::Text(saved.content.clone()));
            fallback.reasoning_content = saved.reasoning.clone();
            fallback.provider_content = saved.provider_content.clone();
        }
        fallback
    }
}

/// 【助手历史】【建表迁移】创建独立原文表，轮次删除时级联清理；参数为连接，返回建表结果。
pub(super) fn create_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS provider_assistant_messages (
            turn_id TEXT NOT NULL REFERENCES turns(turn_id) ON DELETE CASCADE,
            message_key TEXT NOT NULL,
            content_json TEXT NOT NULL,
            PRIMARY KEY (turn_id, message_key)
        );",
    )?;
    Ok(())
}

impl StateStore {
    /// 【助手历史】【保存原文】保存一个已接收的助手子轮，不依赖可见文本再生成协议块。
    /// @param turn_id 为轮次；key 为消息位置；result 为供应商完整结果
    /// @returns 持久化结果
    pub(crate) fn save_assistant_message(
        &self,
        turn_id: &str,
        key: AssistantMessageKey<'_>,
        result: &ChatResult,
    ) -> Result<()> {
        let saved = StoredAssistantMessage {
            content: result.content.clone(),
            reasoning: result.reasoning.clone(),
            provider_content: result.provider_content.clone(),
        };
        self.conv_db.conn.lock().unwrap().execute(
            "INSERT INTO provider_assistant_messages (turn_id, message_key, content_json)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(turn_id, message_key) DO UPDATE SET content_json = excluded.content_json",
            params![turn_id, key.encoded(), serde_json::to_string(&saved)?],
        )?;
        Ok(())
    }
}

/// 【助手历史】【轮次读取】获取一个轮次的原文；参数为数据库与轮次标识，返回可重放集合。
pub(super) fn load(db: &ConversationDb, turn_id: &str) -> Result<SavedAssistantMessages> {
    let conn = db.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT message_key, content_json FROM provider_assistant_messages WHERE turn_id = ?1",
    )?;
    let rows = stmt.query_map(params![turn_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut messages = HashMap::new();
    for row in rows {
        let (key, value) = row?;
        messages.insert(key, serde_json::from_str(&value)?);
    }
    Ok(SavedAssistantMessages(messages))
}

#[cfg(test)]
#[path = "assistant_messages_tests.rs"]
mod tests;
