use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnStatus {
    Running,
    Completed,
    Interrupted,
}

impl TurnStatus {
    /// 转换为数据库状态文本。
    ///
    /// 返回:
    /// - 状态文本
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Interrupted => "interrupted",
        }
    }

    /// 从数据库状态文本恢复状态枚举。
    ///
    /// 参数:
    /// - `value`: 数据库状态文本
    ///
    /// 返回:
    /// - 状态枚举
    pub(super) fn from_str(value: &str) -> Self {
        match value {
            "completed" => Self::Completed,
            "interrupted" => Self::Interrupted,
            _ => Self::Running,
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Turn {
    pub turn_id: String,
    pub seq: i64,
    pub user_content: String,
    pub user_timestamp: String,
    pub assistant_content: String,
    pub assistant_reasoning: Option<String>,
    pub assistant_timestamp: Option<String>,
    pub status: TurnStatus,
    pub tool_reports: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StoredConversationEntry {
    pub timestamp: String,
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub reasoning: Option<String>,
}

/// 返回运行中占位文本。
///
/// 返回:
/// - 运行中占位文本
#[cfg(test)]
pub fn pending_placeholder() -> &'static str {
    ""
}

/// 将轮次列表转换为旧消息入口视图。
///
/// 参数:
/// - `turns`: 轮次列表
///
/// 返回:
/// - 按 user/assistant 展开的消息入口
pub fn turns_to_entries(turns: Vec<Turn>) -> Vec<StoredConversationEntry> {
    let mut entries = Vec::with_capacity(turns.len() * 3);
    for turn in turns {
        let assistant_timestamp = turn.assistant_timestamp.clone().unwrap_or_default();
        entries.push(StoredConversationEntry {
            timestamp: turn.user_timestamp,
            role: "user".to_string(),
            content: turn.user_content,
            reasoning: None,
        });
        if !turn.assistant_content.is_empty() || turn.assistant_reasoning.is_some() {
            entries.push(StoredConversationEntry {
                timestamp: assistant_timestamp.clone(),
                role: "assistant".to_string(),
                content: turn.assistant_content,
                reasoning: turn.assistant_reasoning,
            });
        }
        for report in turn.tool_reports {
            entries.push(StoredConversationEntry {
                timestamp: assistant_timestamp.clone(),
                role: "assistant".to_string(),
                content: report,
                reasoning: None,
            });
        }
    }
    entries
}
