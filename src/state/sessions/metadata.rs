use super::super::model::{SessionInfo, DEFAULT_SESSION_ID};
use chrono::Utc;

/// 生成新会话 ID。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 会话 ID
pub(super) fn new_session_id() -> String {
    format!(
        "session_{}_{}",
        Utc::now().timestamp_millis(),
        rand::random::<u16>()
    )
}

/// 清理会话 ID。
///
/// 参数:
/// - `session_id`: 原始会话 ID
///
/// 返回:
/// - 安全会话 ID
pub(in crate::state::sessions) fn sanitize_session_id(session_id: &str) -> String {
    let value = session_id
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
        .collect::<String>();
    if value.is_empty() {
        DEFAULT_SESSION_ID.to_string()
    } else {
        value
    }
}

/// 从用户消息生成会话标题。
///
/// 参数:
/// - `message`: 用户消息
/// - `fallback`: 默认标题
///
/// 返回:
/// - 会话标题
pub(super) fn title_from_message(message: &str, fallback: &str) -> String {
    let title = message
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(32)
        .collect::<String>();
    if title.trim().is_empty() {
        fallback.to_string()
    } else {
        title
    }
}

/// 按更新时间排序会话。
///
/// 参数:
/// - `sessions`: 会话列表
///
/// 返回:
/// - 无
pub(in crate::state::sessions) fn sort_sessions(sessions: &mut [SessionInfo]) {
    sessions.sort_by(|a, b| {
        if a.id == DEFAULT_SESSION_ID {
            std::cmp::Ordering::Greater
        } else if b.id == DEFAULT_SESSION_ID {
            std::cmp::Ordering::Less
        } else {
            b.updated_at.cmp(&a.updated_at)
        }
    });
}
