use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::{bail, Result};

/// 创建并切换到新会话。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `title`: 可选会话标题文本
///
/// 返回:
/// - 创建结果文本
pub fn create_new_session(paths: &SaiPaths, title: &str) -> Result<String> {
    let title = title.trim();
    let session = if title.is_empty() {
        crate::state::create_session(paths, None)?
    } else {
        crate::state::create_session(paths, Some(title))?
    };
    Ok(format!(
        "{}: {}  {}",
        t("created session", "已创建会话"),
        session.id,
        session.title
    ))
}

/// 切换到指定会话（resume）。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `session_id`: 目标会话 ID
///
/// 返回:
/// - 切换结果文本
pub fn resume_session(paths: &SaiPaths, session_id: &str) -> Result<String> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        bail!("{}", t("session id is required", "需要提供会话 ID"));
    }
    let session = crate::state::switch_session(paths, session_id)?;
    Ok(format!(
        "{}: {}  {}",
        t("current session", "当前会话"),
        session.id,
        session.title
    ))
}

/// 重命名当前会话。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `title`: 新标题
///
/// 返回:
/// - 重命名结果文本
pub fn rename_current_session(paths: &SaiPaths, title: &str) -> Result<String> {
    let title = title.trim();
    if title.is_empty() {
        bail!("{}", t("session title is required", "需要提供会话标题"));
    }
    let active = crate::state::active_session(paths)?;
    let session = crate::state::rename_session(paths, &active.id, title)?;
    Ok(format!(
        "{}: {}  {}",
        t("renamed session", "已重命名会话"),
        session.id,
        session.title
    ))
}

/// 列出可供 resume 选择的会话标签。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - `(会话 ID, 展示文本)` 列表，按 `list_sessions` 顺序
pub fn session_resume_choices(paths: &SaiPaths) -> Result<Vec<(String, String)>> {
    let active = crate::state::active_session(paths)?;
    let sessions = crate::state::list_sessions(paths)?;
    if sessions.is_empty() {
        bail!("{}", t("no sessions available", "没有可用会话"));
    }
    Ok(sessions
        .into_iter()
        .map(|session| {
            let marker = if session.id == active.id { "*" } else { " " };
            let holder = session_loaded_owner(paths, &session.id)
                .map(|owner| holder_label(&owner))
                .unwrap_or_default();
            // 选择列表用短 ID、相对时间与加载态，完整 ID 仍作为返回值供切换使用
            let label = format!(
                "{marker} {:<8}  {:<10}  {:<6}  {}",
                short_session_id(&session.id),
                relative_time(&session.updated_at),
                holder,
                session.title
            );
            (session.id, label)
        })
        .collect())
}

/// 读取会话存活持有者类型。
///
/// 终端、网页或网关打开该会话期间会写心跳；过期或无登记视为未加载。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `session_id`: 会话 ID
///
/// 返回:
/// - 存活持有者类型；未加载时为空
fn session_loaded_owner(paths: &SaiPaths, session_id: &str) -> Option<String> {
    let Ok((_, state_dir)) = crate::state::locate_session_dirs(paths, session_id) else {
        return None;
    };
    let record = crate::runner::session_holder(&state_dir)?;
    crate::runner::holder_is_alive(&record).then_some(record.owner)
}

/// 把持有者类型收成列表里的短标签。
///
/// 参数:
/// - `owner`: `repl` / `web` / `gateway` 等
///
/// 返回:
/// - 如 `[终端]`；未知类型带方括号原样展示
fn holder_label(owner: &str) -> String {
    match owner {
        "repl" => t("[term]", "[终端]").to_string(),
        "web" => t("[web]", "[网页]").to_string(),
        "gateway" => t("[gw]", "[网关]").to_string(),
        other => format!("[{other}]"),
    }
}

/// 生成会话短 ID（取 ID 尾段）。
///
/// 参数:
/// - `id`: 完整会话 ID，如 `session_1785067284416_32368`
///
/// 返回:
/// - 如 `#32368`；无下划线分段时原样返回
fn short_session_id(id: &str) -> String {
    match id.rsplit('_').next() {
        Some(tail) if tail != id => format!("#{tail}"),
        _ => id.to_string(),
    }
}

/// 将 RFC3339 时间转为相对时间描述。
///
/// 参数:
/// - `updated_at`: RFC3339 时间文本
///
/// 返回:
/// - 如 `3 小时前`；解析失败时原样返回
pub fn relative_time(updated_at: &str) -> String {
    let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(updated_at) else {
        return updated_at.to_string();
    };
    let delta = chrono::Utc::now().signed_duration_since(parsed.with_timezone(&chrono::Utc));
    let seconds = delta.num_seconds();
    if seconds < 0 {
        return updated_at.chars().take(10).collect();
    }
    if seconds < 60 {
        return t("just now", "刚刚").to_string();
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes} {}", t("min ago", "分钟前"));
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours} {}", t("hr ago", "小时前"));
    }
    let days = hours / 24;
    if days < 30 {
        return format!("{days} {}", t("days ago", "天前"));
    }
    // 超过一个月直接显示日期，避免不精确的月/年换算
    updated_at.chars().take(10).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::SaiPaths;
    use std::path::PathBuf;
    use tempfile::tempdir;

    /// 构造测试用路径。
    fn test_paths(root: PathBuf) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.jsonc"),
            skills_dir: root.join("config/skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish/sai.fish"),
            bash_hook_file: root.join("shell/bash-hook.sh"),
            zsh_hook_file: root.join("shell/zsh-hook.zsh"),
            powershell_hook_file: root.join("shell/powershell-hook.ps1"),
        }
    }

    #[test]
    fn resume_session_switches_active_session() {
        let dir = tempdir().unwrap();
        let paths = test_paths(dir.path().to_path_buf());
        let created = crate::state::create_session(&paths, Some("alpha")).unwrap();
        let _ = crate::state::create_session(&paths, Some("beta")).unwrap();
        let message = resume_session(&paths, &created.id).unwrap();
        assert!(message.contains(&created.id));
        let active = crate::state::active_session(&paths).unwrap();
        assert_eq!(active.id, created.id);
    }

    #[test]
    fn session_resume_choices_marks_active_session() {
        let dir = tempdir().unwrap();
        let paths = test_paths(dir.path().to_path_buf());
        let _ = crate::state::create_session(&paths, Some("work")).unwrap();
        let choices = session_resume_choices(&paths).unwrap();
        assert!(choices.iter().any(|(_, label)| label.starts_with('*')));
        assert!(choices.len() >= 2);
    }

    /// 【TUI】【恢复会话】已打开的会话在列表里标出终端/网页/网关，未打开的不标。
    #[test]
    fn session_resume_choices_marks_loaded_holder() {
        let dir = tempdir().unwrap();
        let paths = test_paths(dir.path().to_path_buf());
        let held = crate::state::create_session(&paths, Some("held")).unwrap();
        let idle = crate::state::create_session(&paths, Some("idle")).unwrap();
        let (_, state_dir) = crate::state::locate_session_dirs(&paths, &held.id).unwrap();
        let _guard = crate::runner::SessionHolderGuard::acquire(
            &state_dir,
            &held.id,
            crate::runner::SessionOwner::Web,
        )
        .unwrap();

        let choices = session_resume_choices(&paths).unwrap();
        let held_label = choices
            .iter()
            .find(|(id, _)| id == &held.id)
            .map(|(_, label)| label.as_str())
            .unwrap();
        let idle_label = choices
            .iter()
            .find(|(id, _)| id == &idle.id)
            .map(|(_, label)| label.as_str())
            .unwrap();
        assert!(
            held_label.contains("[网页]") || held_label.contains("[web]"),
            "{held_label}"
        );
        assert!(
            !idle_label.contains("[网页]")
                && !idle_label.contains("[web]")
                && !idle_label.contains("[终端]")
                && !idle_label.contains("[term]"),
            "{idle_label}"
        );
    }

    #[test]
    fn short_id_and_relative_time_render_compactly() {
        assert_eq!(short_session_id("session_1785067284416_32368"), "#32368");
        assert_eq!(short_session_id("default"), "default");

        let recent = chrono::Utc::now() - chrono::Duration::minutes(5);
        let rendered = relative_time(&recent.to_rfc3339());
        assert!(rendered.contains('5'));
        assert!(rendered.contains("分钟前") || rendered.contains("min ago"));
        // 无法解析的时间原样返回
        assert_eq!(relative_time("not-a-date"), "not-a-date");
    }
}
