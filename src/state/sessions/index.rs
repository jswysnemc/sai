use super::model::{SessionInfo, DEFAULT_SESSION_ID};
use super::repository::sort_sessions;
use super::repository_paths::{current_session_file, sessions_file};
use anyhow::{Context, Result};
use chrono::Utc;
use std::path::Path;

#[cfg(test)]
thread_local! {
    static INDEX_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static INDEX_WRITES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// 【会话索引】【性能回归】读取当前测试线程累计的索引访问次数。
/// @returns (读取次数, 写入次数)；无参数
#[cfg(test)]
pub(crate) fn session_index_io_counts() -> (usize, usize) {
    (
        INDEX_READS.with(std::cell::Cell::get),
        INDEX_WRITES.with(std::cell::Cell::get),
    )
}

/// 【会话索引】【默认会话】确保默认会话存在，已有索引保持只读。
///
/// 参数:
/// - `base_state_dir`: 工作区会话作用域目录
///
/// 返回:
/// - 会话列表
pub(super) fn ensure_default_session_for_base(base_state_dir: &Path) -> Result<Vec<SessionInfo>> {
    std::fs::create_dir_all(base_state_dir)?;
    let mut sessions = read_sessions_from_base(base_state_dir)?;
    let missing_default = !sessions
        .iter()
        .any(|session| session.id == DEFAULT_SESSION_ID);
    if missing_default {
        let now = Utc::now().to_rfc3339();
        sessions.push(SessionInfo::default_with_time(&now));
    }
    sort_sessions(&mut sessions);
    // 1. 【会话索引】【只读列表】已有索引只在需要补默认会话时写入
    if missing_default {
        save_sessions_to_base(base_state_dir, &sessions)?;
    }
    Ok(sessions)
}

/// 【会话索引】【活动指针】读取当前会话 ID。
///
/// 参数:
/// - `base_state_dir`: 工作区会话作用域目录
///
/// 返回:
/// - 当前会话 ID
pub(super) fn read_current_session_id_from_base(base_state_dir: &Path) -> Result<String> {
    let file = current_session_file(base_state_dir);
    if !file.exists() {
        write_current_session_id_to_base(base_state_dir, DEFAULT_SESSION_ID)?;
        return Ok(DEFAULT_SESSION_ID.to_string());
    }
    let value = std::fs::read_to_string(file)?;
    Ok(value.trim().to_string())
}

/// 【会话索引】【活动指针】写入当前会话 ID。
///
/// 参数:
/// - `base_state_dir`: 工作区会话作用域目录
/// - `session_id`: 会话 ID
///
/// 返回:
/// - 写入是否成功
pub(super) fn write_current_session_id_to_base(
    base_state_dir: &Path,
    session_id: &str,
) -> Result<()> {
    if let Some(parent) = current_session_file(base_state_dir).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        current_session_file(base_state_dir),
        format!("{session_id}\n"),
    )?;
    Ok(())
}

/// 【会话索引】【读取】读取会话索引。
///
/// 参数:
/// - `base_state_dir`: 原始状态目录
///
/// 返回:
/// - 会话列表
pub(super) fn read_sessions_from_base(base_state_dir: &Path) -> Result<Vec<SessionInfo>> {
    #[cfg(test)]
    INDEX_READS.with(|count| count.set(count.get() + 1));
    let file = sessions_file(base_state_dir);
    if !file.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&file)
        .with_context(|| format!("failed to read {}", file.display()))?;
    Ok(
        serde_json::from_str(&raw)
            .with_context(|| format!("invalid JSON in {}", file.display()))?,
    )
}

/// 【会话索引】【写入】保存会话索引。
///
/// 参数:
/// - `base_state_dir`: 原始状态目录
/// - `sessions`: 会话列表
///
/// 返回:
/// - 保存是否成功
pub(super) fn save_sessions_to_base(base_state_dir: &Path, sessions: &[SessionInfo]) -> Result<()> {
    #[cfg(test)]
    INDEX_WRITES.with(|count| count.set(count.get() + 1));
    let file = sessions_file(base_state_dir);
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        file,
        format!("{}\n", serde_json::to_string_pretty(sessions)?),
    )?;
    Ok(())
}
