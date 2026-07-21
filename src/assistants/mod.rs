//! 轻量助手：会话标题、Git 提交说明等短任务模型调用。

mod commit_message;
mod session_title;

pub(crate) use commit_message::{
    collect_repo_change_summary, generate_commit_message, resolve_commit_message_client,
};
pub(crate) use session_title::maybe_auto_title_session;
