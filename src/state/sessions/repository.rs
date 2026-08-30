use super::model::{LocatedSession, SessionInfo, DEFAULT_SESSION_ID};
use super::repository_paths::{current_session_file, sessions_file};
use super::workspace::{current_workspace_scope, WorkspaceScope};
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use chrono::Utc;
use std::path::{Path, PathBuf};

#[path = "metadata.rs"]
mod metadata;
use metadata::{new_session_id, title_from_message};
pub(super) use metadata::{sanitize_session_id, sort_sessions};

/// 从用户消息生成会话标题（供自动命名失败时回退）。
///
/// 参数:
/// - `message`: 用户消息
/// - `fallback`: 默认标题
///
/// 返回:
/// - 会话标题
pub fn title_from_message_public(message: &str, fallback: &str) -> String {
    title_from_message(message, fallback)
}

/// 创建新会话并设为当前会话。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `title`: 可选会话标题
///
/// 返回:
/// - 新会话信息
pub fn create_session(paths: &SaiPaths, title: Option<&str>) -> Result<SessionInfo> {
    let scope = current_session_scope(paths)?;
    create_session_in_scope(&scope, title)
}

/// 在指定工作区创建新会话并设为该工作区当前会话。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `workspace_path`: 工作区目录
/// - `title`: 可选会话标题
///
/// 返回:
/// - 新会话信息
pub fn create_session_for_workspace(
    paths: &SaiPaths,
    workspace_path: &Path,
    title: Option<&str>,
) -> Result<SessionInfo> {
    let scope = super::workspace::workspace_scope_for_path(paths, workspace_path);
    create_session_in_scope(&scope, title)
}

/// 在指定会话作用域创建并切换会话。
///
/// 参数:
/// - `scope`: 工作区会话作用域
/// - `title`: 可选会话标题
///
/// 返回:
/// - 新会话信息
fn create_session_in_scope(scope: &WorkspaceScope, title: Option<&str>) -> Result<SessionInfo> {
    let now = Utc::now().to_rfc3339();
    let session = SessionInfo {
        id: new_session_id(),
        title: title
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| "New session".to_string()),
        created_at: now.clone(),
        updated_at: now,
    };
    let mut sessions = ensure_default_session_for_base(&scope.state_dir)?;
    sessions.insert(0, session.clone());
    save_sessions_to_base(&scope.state_dir, &sessions)?;
    write_current_session_id_to_base(&scope.state_dir, &session.id)?;
    std::fs::create_dir_all(session_state_dir(&scope.state_dir, &session.id))?;
    Ok(session)
}

/// 切换当前会话。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `session_id`: 会话 ID
///
/// 返回:
/// - 当前会话信息
pub fn switch_session(paths: &SaiPaths, session_id: &str) -> Result<SessionInfo> {
    let scope = current_session_scope(paths)?;
    let session_id = session_id.trim();
    let session = ensure_default_session_for_base(&scope.state_dir)?
        .into_iter()
        .find(|session| session.id == session_id)
        .with_context(|| format!("session not found: {session_id}"))?;
    write_current_session_id_to_base(&scope.state_dir, &session.id)?;
    std::fs::create_dir_all(session_state_dir(&scope.state_dir, &session.id))?;
    Ok(session)
}

/// 重命名会话。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `session_id`: 会话 ID
/// - `title`: 新标题
///
/// 返回:
/// - 更新后的会话信息
pub fn rename_session(paths: &SaiPaths, session_id: &str, title: &str) -> Result<SessionInfo> {
    let scope = current_session_scope(paths)?;
    let title = title.trim();
    if title.is_empty() {
        bail!("session title cannot be empty");
    }
    let mut sessions = ensure_default_session_for_base(&scope.state_dir)?;
    let session = sessions
        .iter_mut()
        .find(|session| session.id == session_id.trim())
        .with_context(|| format!("session not found: {}", session_id.trim()))?;
    session.title = title.to_string();
    session.updated_at = Utc::now().to_rfc3339();
    let updated = session.clone();
    sort_sessions(&mut sessions);
    save_sessions_to_base(&scope.state_dir, &sessions)?;
    Ok(updated)
}

/// 删除会话。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `session_id`: 会话 ID
///
/// 返回:
/// - 是否删除了会话
pub fn delete_session(paths: &SaiPaths, session_id: &str) -> Result<bool> {
    Ok(!delete_sessions(paths, &[session_id.to_string()])?.is_empty())
}

/// 批量删除会话并仅写入一次索引。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `session_ids`: 待删除会话 ID 列表
///
/// 返回:
/// - 实际删除的会话 ID 列表
pub fn delete_sessions(paths: &SaiPaths, session_ids: &[String]) -> Result<Vec<String>> {
    let scope = current_session_scope(paths)?;
    delete_sessions_in_base(&scope.state_dir, session_ids)
}

/// 在指定会话作用域内删除会话并仅写入一次索引。
///
/// 会话按工作区分目录存放，同名 ID（例如各工作区都有的 default）只在自己的
/// 作用域里有效。删除必须落到会话真正所属的作用域，否则索引里找不到这个 ID，
/// 会静默地一个都删不掉。
///
/// 参数:
/// - `state_dir`: 会话作用域的状态目录
/// - `session_ids`: 待删除会话 ID 列表
///
/// 返回:
/// - 实际删除的会话 ID 列表
pub(super) fn delete_sessions_in_base(
    state_dir: &Path,
    session_ids: &[String],
) -> Result<Vec<String>> {
    let requested = session_ids
        .iter()
        .map(|id| id.trim())
        .filter(|id| !id.is_empty())
        .collect::<std::collections::BTreeSet<_>>();
    let mut sessions = read_sessions_from_base(state_dir)?;
    let deleted = sessions
        .iter()
        .filter(|session| requested.contains(session.id.as_str()))
        .map(|session| session.id.clone())
        .collect::<Vec<_>>();
    if deleted.is_empty() {
        return Ok(Vec::new());
    }
    // 1. 允许删除任意会话（含 default），不再强制保留至少一个
    sessions.retain(|session| !requested.contains(session.id.as_str()));
    save_sessions_to_base(state_dir, &sessions)?;
    for session_id in &deleted {
        let session_dir = session_state_dir(state_dir, session_id);
        if session_dir.exists() {
            std::fs::remove_dir_all(session_dir)?;
        }
    }
    // 2. 当前会话被删时，切到剩余最新会话；若已删空则写入空标记，下一次 ensure 再补默认
    let current = read_current_session_id_from_base(state_dir)?;
    if deleted.contains(&current) {
        if let Some(fallback) = sessions.first() {
            write_current_session_id_to_base(state_dir, &fallback.id)?;
        } else {
            // 索引已空：清理 current，避免指向已删除目录
            let _ = std::fs::remove_file(current_session_file(state_dir));
        }
    }
    Ok(deleted)
}

/// 确保当前会话存在。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 当前会话信息
pub fn ensure_active_session(paths: &SaiPaths) -> Result<SessionInfo> {
    let scope = current_session_scope(paths)?;
    let sessions = ensure_default_session_for_base(&scope.state_dir)?;
    let active_id = read_current_session_id_from_base(&scope.state_dir)?;
    if let Some(session) = sessions.iter().find(|session| session.id == active_id) {
        std::fs::create_dir_all(session_state_dir(&scope.state_dir, &session.id))?;
        return Ok(session.clone());
    }
    write_current_session_id_to_base(&scope.state_dir, DEFAULT_SESSION_ID)?;
    let session = sessions
        .into_iter()
        .find(|session| session.id == DEFAULT_SESSION_ID)
        .expect("default session must exist");
    std::fs::create_dir_all(session_state_dir(&scope.state_dir, &session.id))?;
    Ok(session)
}

/// 返回当前会话状态目录。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 当前会话状态目录
pub fn active_state_dir(paths: &SaiPaths) -> Result<PathBuf> {
    let scope = current_session_scope(paths)?;
    let session = ensure_active_session(paths)?;
    Ok(session_state_dir(&scope.state_dir, &session.id))
}

/// 返回指定会话的状态目录，并校验会话存在。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `session_id`: 会话 ID
///
/// 返回:
/// - 指定会话状态目录
#[allow(dead_code)]
pub fn state_dir_for_session(paths: &SaiPaths, session_id: &str) -> Result<PathBuf> {
    Ok(locate_session_dirs(paths, session_id)?.1)
}

/// 定位会话所属工作区目录与会话状态目录。
///
/// 优先当前工作区；找不到时再扫描其它工作区索引，避免跨工作区读取 timeline 时 404。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `session_id`: 会话 ID
///
/// 返回:
/// - `(工作区会话作用域目录, 会话状态目录)`
pub fn locate_session_dirs(paths: &SaiPaths, session_id: &str) -> Result<(PathBuf, PathBuf)> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        bail!("session id cannot be empty");
    }

    // 1. 先查当前工作区（热路径）
    let current = current_session_scope(paths)?;
    if let Some(state_dir) = session_dir_if_present(&current.state_dir, session_id)? {
        return Ok((current.state_dir, state_dir));
    }

    // 2. 扫描其它工作区索引，只读 index.json，不创建默认会话
    let workspaces_root = paths.state_dir.join("sessions").join("workspaces");
    if workspaces_root.is_dir() {
        for entry in std::fs::read_dir(&workspaces_root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let base = entry.path();
            if base == current.state_dir {
                continue;
            }
            if let Some(state_dir) = session_dir_if_present(&base, session_id)? {
                return Ok((base, state_dir));
            }
        }
    }

    Err(anyhow::anyhow!("session not found: {session_id}"))
}

/// 只读列出所有工作区的会话。
///
/// 与 `list_sessions` 不同：既不创建索引文件也不补默认会话，索引缺失的工作区
/// 直接跳过。探测类工具要能在不留下任何痕迹的前提下观察别的会话。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 按工作区、会话 ID 排序的会话及其状态目录
pub fn list_all_sessions(paths: &SaiPaths) -> Result<Vec<LocatedSession>> {
    let workspaces_root = paths.state_dir.join("sessions").join("workspaces");
    let entries = match std::fs::read_dir(&workspaces_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read {}", workspaces_root.display()))
        }
    };
    let mut located = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let base = entry.path();
        let workspace_id = entry.file_name().to_string_lossy().into_owned();
        let current = current_session_id_if_present(&base);
        for session in read_sessions_from_base(&base)? {
            located.push(LocatedSession {
                state_dir: session_state_dir(&base, &session.id),
                is_current: session.id == current,
                info: session,
                workspace_id: workspace_id.clone(),
            });
        }
    }
    located.sort_by(|left, right| {
        left.workspace_id
            .cmp(&right.workspace_id)
            .then_with(|| left.info.id.cmp(&right.info.id))
    });
    Ok(located)
}

/// 读取当前会话 ID，文件缺失时按默认会话处理且不写盘。
///
/// 参数:
/// - `base_state_dir`: 工作区会话作用域目录
///
/// 返回:
/// - 当前会话 ID
fn current_session_id_if_present(base_state_dir: &Path) -> String {
    let file = current_session_file(base_state_dir);
    if !file.is_file() {
        return DEFAULT_SESSION_ID.to_string();
    }
    std::fs::read_to_string(file)
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|_| DEFAULT_SESSION_ID.to_string())
}

/// 若索引中存在该会话则返回会话状态目录。
fn session_dir_if_present(base_state_dir: &Path, session_id: &str) -> Result<Option<PathBuf>> {
    let sessions = read_sessions_from_base(base_state_dir)?;
    if !sessions.iter().any(|session| session.id == session_id) {
        return Ok(None);
    }
    let state_dir = session_state_dir(base_state_dir, session_id);
    std::fs::create_dir_all(&state_dir)?;
    Ok(Some(state_dir))
}

/// 返回当前工作区会话作用域目录。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 当前工作区会话作用域目录
pub fn session_scope_dir(paths: &SaiPaths) -> Result<PathBuf> {
    Ok(current_session_scope(paths)?.state_dir)
}

/// 返回完成旧状态迁移后的当前工作区会话作用域。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 当前工作区会话作用域
pub(super) fn current_session_scope(paths: &SaiPaths) -> Result<WorkspaceScope> {
    let scope = current_workspace_scope(paths)?;
    migrate_legacy_sessions_to_workspace(paths, &scope.state_dir)?;
    Ok(scope)
}

/// 首次使用工作区会话时迁移旧版全局会话状态。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `workspace_state_dir`: 当前工作区会话作用域目录
///
/// 返回:
/// - 迁移是否成功
pub(super) fn migrate_legacy_sessions_to_workspace(
    paths: &SaiPaths,
    workspace_state_dir: &Path,
) -> Result<()> {
    if workspace_has_state(workspace_state_dir)? {
        return Ok(());
    }

    let legacy_sessions_dir = paths.state_dir.join("sessions");
    let legacy_index = legacy_sessions_dir.join("index.json");
    let legacy_current = legacy_sessions_dir.join("current");
    let legacy_data_dir = legacy_sessions_dir.join("data");
    let has_legacy_sessions =
        legacy_index.exists() || legacy_current.exists() || legacy_data_dir.exists();
    let has_legacy_default = legacy_default_files()
        .iter()
        .any(|name| paths.state_dir.join(name).exists());
    if !has_legacy_sessions && !has_legacy_default {
        return Ok(());
    }

    std::fs::create_dir_all(workspace_state_dir)?;
    copy_file_if_missing(&legacy_index, &sessions_file(workspace_state_dir))?;
    copy_file_if_missing(&legacy_current, &current_session_file(workspace_state_dir))?;
    copy_dir_contents_if_missing(&legacy_data_dir, &workspace_state_dir.join("data"))?;

    if has_legacy_default {
        let default_dir = session_state_dir(workspace_state_dir, DEFAULT_SESSION_ID);
        std::fs::create_dir_all(&default_dir)?;
        for name in legacy_default_files() {
            copy_file_if_missing(&paths.state_dir.join(name), &default_dir.join(name))?;
        }
    }
    Ok(())
}

/// 判断工作区会话作用域是否已经存在有效状态。
///
/// 参数:
/// - `workspace_state_dir`: 当前工作区会话作用域目录
///
/// 返回:
/// - 存在状态时返回 true
fn workspace_has_state(workspace_state_dir: &Path) -> Result<bool> {
    if sessions_file(workspace_state_dir).exists()
        || current_session_file(workspace_state_dir).exists()
    {
        return Ok(true);
    }
    has_dir_entries(&workspace_state_dir.join("data"))
}

/// 返回旧版默认会话状态文件名。
///
/// 返回:
/// - 文件名列表
fn legacy_default_files() -> &'static [&'static str] {
    &[
        "conversation.db",
        "conversation.jsonl",
        "usage.json",
        "loaded-tools.json",
        "sai.log",
        "profile.md",
        "compaction-summary.json",
        "prompt.sha256",
    ]
}

/// 判断目录是否存在条目。
///
/// 参数:
/// - `path`: 目录路径
///
/// 返回:
/// - 存在条目时返回 true
fn has_dir_entries(path: &Path) -> Result<bool> {
    if !path.is_dir() {
        return Ok(false);
    }
    Ok(std::fs::read_dir(path)?.next().transpose()?.is_some())
}

/// 复制文件，目标已存在时跳过。
///
/// 参数:
/// - `source`: 源文件路径
/// - `target`: 目标文件路径
///
/// 返回:
/// - 复制是否成功
fn copy_file_if_missing(source: &Path, target: &Path) -> Result<()> {
    if !source.is_file() || target.exists() {
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(source, target)?;
    Ok(())
}

/// 递归复制目录内容，目标已存在的条目会跳过。
///
/// 参数:
/// - `source`: 源目录路径
/// - `target`: 目标目录路径
///
/// 返回:
/// - 复制是否成功
fn copy_dir_contents_if_missing(source: &Path, target: &Path) -> Result<()> {
    if !source.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if target_path.exists() {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_contents_if_missing(&source_path, &target_path)?;
        } else if file_type.is_file() {
            copy_file_if_missing(&source_path, &target_path)?;
        }
    }
    Ok(())
}

/// 根据用户消息更新会话标题和更新时间。
///
/// 参数:
/// - `base_state_dir`: 原始状态目录
/// - `session_id`: 会话 ID
/// - `message`: 用户消息
///
/// 返回:
/// - 更新是否成功
pub fn touch_session_with_message(
    base_state_dir: &Path,
    session_id: &str,
    message: &str,
) -> Result<()> {
    let mut sessions = ensure_default_session_for_base(base_state_dir)?;
    let now = Utc::now().to_rfc3339();
    if let Some(session) = sessions.iter_mut().find(|session| session.id == session_id) {
        if session.title == "New session" || session.title == "Default" {
            session.title = title_from_message(message, &session.title);
        }
        session.updated_at = now;
    }
    sort_sessions(&mut sessions);
    save_sessions_to_base(base_state_dir, &sessions)
}

/// 确保默认会话存在。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 会话列表
pub(super) fn ensure_default_session_for_base(base_state_dir: &Path) -> Result<Vec<SessionInfo>> {
    std::fs::create_dir_all(base_state_dir)?;
    let mut sessions = read_sessions_from_base(base_state_dir)?;
    if !sessions
        .iter()
        .any(|session| session.id == DEFAULT_SESSION_ID)
    {
        let now = Utc::now().to_rfc3339();
        sessions.push(SessionInfo::default_with_time(&now));
    }
    sort_sessions(&mut sessions);
    save_sessions_to_base(base_state_dir, &sessions)?;
    Ok(sessions)
}

/// 读取当前会话 ID。
///
/// 参数:
/// - `paths`: Sai 路径
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

/// 写入当前会话 ID。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `session_id`: 会话 ID
///
/// 返回:
/// - 写入是否成功
fn write_current_session_id_to_base(base_state_dir: &Path, session_id: &str) -> Result<()> {
    if let Some(parent) = current_session_file(base_state_dir).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        current_session_file(base_state_dir),
        format!("{session_id}\n"),
    )?;
    Ok(())
}

/// 读取会话索引。
///
/// 参数:
/// - `base_state_dir`: 原始状态目录
///
/// 返回:
/// - 会话列表
fn read_sessions_from_base(base_state_dir: &Path) -> Result<Vec<SessionInfo>> {
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

/// 保存会话索引。
///
/// 参数:
/// - `base_state_dir`: 原始状态目录
/// - `sessions`: 会话列表
///
/// 返回:
/// - 保存是否成功
pub(super) fn save_sessions_to_base(base_state_dir: &Path, sessions: &[SessionInfo]) -> Result<()> {
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

/// 返回会话状态目录。
///
/// 参数:
/// - `base_state_dir`: 当前工作区会话作用域目录
/// - `session_id`: 会话 ID
///
/// 返回:
/// - 会话状态目录
pub(super) fn session_state_dir(base_state_dir: &Path, session_id: &str) -> PathBuf {
    base_state_dir
        .join("data")
        .join(sanitize_session_id(session_id))
}

include!("repository_tests.rs");
