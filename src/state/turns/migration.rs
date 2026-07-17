use super::model::TurnStatus;
use super::repository::{ConversationDb, InsertTurn};
use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Deserialize)]
struct JsonlEntry {
    #[serde(default)]
    timestamp: String,
    #[serde(default)]
    role: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    reasoning: Option<String>,
}

impl ConversationDb {
    /// 从旧 conversation.jsonl 迁移历史记录。
    ///
    /// 参数:
    /// - `jsonl_path`: 旧 JSONL 历史文件
    ///
    /// 返回:
    /// - 迁移的轮次数量
    pub fn migrate_from_jsonl(&self, jsonl_path: &Path) -> Result<usize> {
        if !jsonl_path.exists() || !self.load_turns()?.is_empty() {
            return Ok(0);
        }
        let file = std::fs::File::open(jsonl_path)?;
        let mut migrated = 0usize;
        let mut pending_user: Option<(String, String)> = None;
        for line in BufReader::new(file).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let Ok(entry) = serde_json::from_str::<JsonlEntry>(&line) else {
                continue;
            };
            match entry.role.as_str() {
                "user" => {
                    if let Some((timestamp, content)) = pending_user.take() {
                        migrated +=
                            self.insert_migrated_without_reply(migrated, &timestamp, &content)?;
                    }
                    pending_user = Some((entry.timestamp, entry.content));
                }
                "assistant" => {
                    if let Some((user_timestamp, user_content)) = pending_user.take() {
                        migrated += self.insert_migrated_completed(
                            migrated,
                            &user_timestamp,
                            &user_content,
                            &entry.content,
                            entry.reasoning.as_deref(),
                        )?;
                    }
                }
                _ => {}
            }
        }
        if let Some((timestamp, content)) = pending_user {
            migrated += self.insert_migrated_interrupted(migrated, &timestamp, &content)?;
        }
        Ok(migrated)
    }

    /// 写入没有助手回复的旧用户消息。
    ///
    /// 参数:
    /// - `index`: 迁移序号
    /// - `user_timestamp`: 用户消息时间
    /// - `user_content`: 用户消息内容
    ///
    /// 返回:
    /// - 写入轮次数量
    fn insert_migrated_without_reply(
        &self,
        index: usize,
        user_timestamp: &str,
        user_content: &str,
    ) -> Result<usize> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        self.insert_turn_locked(
            &conn,
            InsertTurn {
                turn_id: &format!("migrated_{index}"),
                user_content,
                user_timestamp,
                assistant_content: "(migrated without reply)",
                assistant_reasoning: None,
                assistant_timestamp: Some(&now),
                status: TurnStatus::Interrupted,
                tool_reports: &[],
            },
        )?;
        Ok(1)
    }

    /// 写入完整旧对话轮次。
    ///
    /// 参数:
    /// - `index`: 迁移序号
    /// - `user_timestamp`: 用户消息时间
    /// - `user_content`: 用户消息内容
    /// - `assistant_content`: 助手消息内容
    /// - `reasoning`: 可选推理内容
    ///
    /// 返回:
    /// - 写入轮次数量
    fn insert_migrated_completed(
        &self,
        index: usize,
        user_timestamp: &str,
        user_content: &str,
        assistant_content: &str,
        reasoning: Option<&str>,
    ) -> Result<usize> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        self.insert_turn_locked(
            &conn,
            InsertTurn {
                turn_id: &format!("migrated_{index}"),
                user_content,
                user_timestamp,
                assistant_content,
                assistant_reasoning: reasoning,
                assistant_timestamp: Some(&now),
                status: TurnStatus::Completed,
                tool_reports: &[],
            },
        )?;
        Ok(1)
    }

    /// 写入旧的未完成用户消息。
    ///
    /// 参数:
    /// - `index`: 迁移序号
    /// - `user_timestamp`: 用户消息时间
    /// - `user_content`: 用户消息内容
    ///
    /// 返回:
    /// - 写入轮次数量
    fn insert_migrated_interrupted(
        &self,
        _index: usize,
        _user_timestamp: &str,
        _user_content: &str,
    ) -> Result<usize> {
        Ok(0)
    }
}
