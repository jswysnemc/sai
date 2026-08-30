mod agent_override;
mod assembler;
mod checkpoint;
mod event;
#[cfg(test)]
mod identified_tool_tests;
mod journal;
mod manager;
pub(crate) mod model_override;
mod request_limits;

pub(crate) use assembler::EventAssembler;
pub(crate) use event::WebEvent;
pub(crate) use journal::EventJournal;
pub(crate) use manager::{ActiveRunInfo, QueuedRunUpdate, RunKind, RunManager, StartRunRequest};
pub(crate) use request_limits::MAX_RUN_REQUEST_BYTES;

/// 会话事件日志的路径。
///
/// TUI 与 Web 必须算出同一个路径：它是共享事件文件的唯一约定，两边各自
/// 拼一遍迟早会漂移。文件名的字符白名单与 `EventJournal` 的落盘目录一致。
///
/// 参数:
/// - `state_dir`: Sai 状态根目录
/// - `workspace_id`: 工作区标识
/// - `session_id`: 会话标识
///
/// 返回:
/// - JSONL 事件文件路径
pub(crate) fn session_event_path(
    state_dir: &std::path::Path,
    workspace_id: &str,
    session_id: &str,
) -> std::path::PathBuf {
    state_dir
        .join("web")
        .join("session-events")
        .join(format!("{}.jsonl", sanitize_key(&format!("{workspace_id}:{session_id}"))))
}

/// 把调度键转成跨平台安全的文件名。
///
/// 参数:
/// - `key`: 调度键，可能包含 Windows 不允许的路径分隔符或冒号
///
/// 返回:
/// - 仅含字母数字、`-`、`_` 的文件名主干
fn sanitize_key(key: &str) -> String {
    key.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}
