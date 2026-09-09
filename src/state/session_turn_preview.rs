use super::StateStore;
use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

/// 指定轮次的完整可见正文，独立于当前活动分支。
#[derive(Debug, Serialize)]
pub struct SessionTurnPreview {
    pub turn_id: String,
    pub user: String,
    pub assistant: String,
    pub messages: Vec<TurnPreviewMessage>,
}

/// 轮次进行过程中保存的插入消息。
#[derive(Debug, Serialize)]
pub struct TurnPreviewMessage {
    pub id: String,
    pub role: String,
    pub content: String,
}

impl StateStore {
    /// 【会话分支】【消息预览】读取指定轮次完整正文，不改变活动叶子。
    /// 参数：`turn_id` 为目标轮次；返回：正文及轮次内消息，轮次不存在时为空。
    pub fn session_turn_preview(&self, turn_id: &str) -> Result<Option<SessionTurnPreview>> {
        // 1. 按主键读取单轮内容，避免加载整个会话或切换活动分支
        let content = self.conv_db.with_conn(|conn| {
            Ok(conn
                .query_row(
                    "SELECT user_content, assistant_content, error FROM turns WHERE turn_id = ?1",
                    params![turn_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                        ))
                    },
                )
                .optional()?)
        })?;
        let Some((user, assistant, error)) = content else {
            return Ok(None);
        };
        // 2. 使用可见正文，避免把供应商注入内容或错误摘要混入消息复制结果
        let assistant = if error
            .as_deref()
            .is_some_and(|value| value == assistant.trim())
        {
            String::new()
        } else {
            assistant
        };
        let messages = self
            .turn_messages(turn_id)?
            .into_iter()
            .map(|message| TurnPreviewMessage {
                id: message.id,
                role: message.kind.role().to_string(),
                content: message.display_content,
            })
            .collect();
        Ok(Some(SessionTurnPreview {
            turn_id: turn_id.to_string(),
            user,
            assistant,
            messages,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ConversationDb;
    use std::sync::Arc;

    /// 验证预览非活动分支能返回完整内容，且不会改变当前分支。
    /// 参数：无；返回：无。
    #[test]
    fn previews_inactive_turn_without_switching_branch() {
        let root = tempfile::tempdir().unwrap();
        let db = Arc::new(ConversationDb::open(root.path()).unwrap());
        db.start_turn("turn_first", "完整输入".repeat(100).as_str())
            .unwrap();
        db.complete_turn("turn_first", "完整回复".repeat(100).as_str(), None)
            .unwrap();
        db.start_turn("turn_current", "当前输入").unwrap();
        db.complete_turn("turn_current", "当前回复", None).unwrap();
        let store = StateStore {
            plugin_state_root: None,
            base_state_dir: root.path().to_path_buf(),
            session_id: "preview_session".to_string(),
            state_dir: root.path().to_path_buf(),
            conv_db: db,
        };
        store.move_leaf_to_parent("turn_current").unwrap();
        let leaf_before = store.session_tree().unwrap().active_leaf_id;
        let preview = store.session_turn_preview("turn_current").unwrap().unwrap();
        assert_eq!(preview.user, "当前输入");
        assert_eq!(preview.assistant, "当前回复");
        assert_eq!(store.session_tree().unwrap().active_leaf_id, leaf_before);
        assert!(store.session_turn_preview("missing").unwrap().is_none());
        let first = store.session_turn_preview("turn_first").unwrap().unwrap();
        assert_eq!(first.assistant, "完整回复".repeat(100));
    }
}
