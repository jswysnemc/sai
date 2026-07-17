use super::*;

/// 运行会话管理命令。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `args`: 会话命令参数
///
/// 返回:
/// - 命令是否成功
pub(super) fn run_sessions(paths: &SaiPaths, args: SessionsArgs) -> Result<()> {
    match args.command.unwrap_or(SessionsCommand::List) {
        SessionsCommand::List => list_current_sessions(paths),
        SessionsCommand::New(args) => create_current_session(paths, args),
        SessionsCommand::Switch(args) => switch_current_session(paths, args),
        SessionsCommand::Resume(args) => run_resume(paths, args),
        SessionsCommand::Current => print_current_session(paths),
        SessionsCommand::Delete(args) => delete_current_session(paths, args),
        SessionsCommand::Rename(args) => rename_current_session(paths, args),
    }
}

/// 运行 resume：按 ID 或交互模糊选择后切换会话。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `args`: resume 参数
///
/// 返回:
/// - 是否成功
pub(super) fn run_resume(paths: &SaiPaths, args: ResumeArgs) -> Result<()> {
    let session_id = match args
        .id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        Some(id) => id.to_string(),
        None => select_session_id_interactively(paths)?,
    };
    println!(
        "{}",
        crate::control_commands::resume_session(paths, &session_id)?
    );
    Ok(())
}

/// 交互式模糊选择会话 ID。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 选中的会话 ID；取消时返回错误说明已取消
pub(super) fn select_session_id_interactively(paths: &SaiPaths) -> Result<String> {
    let choices = crate::control_commands::session_resume_choices(paths)?;
    let labels = choices
        .iter()
        .map(|(_, label)| label.clone())
        .collect::<Vec<_>>();
    let Some(index) = inline_fuzzy_select(&labels)? else {
        bail!("{}", t("session selection cancelled", "已取消会话选择"));
    };
    choices
        .get(index)
        .map(|(id, _)| id.clone())
        .ok_or_else(|| anyhow::anyhow!("{}", t("invalid session selection", "无效的会话选择")))
}

/// 输出会话列表。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 输出是否成功
fn list_current_sessions(paths: &SaiPaths) -> Result<()> {
    let active = crate::state::active_session(paths)?;
    for session in crate::state::list_sessions(paths)? {
        let marker = if session.id == active.id { "*" } else { " " };
        println!(
            "{marker} {}  {}  {}",
            session.id, session.updated_at, session.title
        );
    }
    Ok(())
}

/// 创建并切换到新会话。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `args`: 新会话参数
///
/// 返回:
/// - 创建是否成功
fn create_current_session(paths: &SaiPaths, args: SessionTitleArgs) -> Result<()> {
    let title = join_message(args.title);
    let session = crate::state::create_session(paths, Some(&title))?;
    println!(
        "{}: {}  {}",
        t("created session", "已创建会话"),
        session.id,
        session.title
    );
    Ok(())
}

/// 切换当前会话。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `args`: 会话 ID 参数
///
/// 返回:
/// - 切换是否成功
fn switch_current_session(paths: &SaiPaths, args: SessionIdArgs) -> Result<()> {
    let session = crate::state::switch_session(paths, &args.id)?;
    println!(
        "{}: {}  {}",
        t("current session", "当前会话"),
        session.id,
        session.title
    );
    Ok(())
}

/// 输出当前会话。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 输出是否成功
fn print_current_session(paths: &SaiPaths) -> Result<()> {
    let session = crate::state::active_session(paths)?;
    println!("{}  {}  {}", session.id, session.updated_at, session.title);
    Ok(())
}

/// 删除会话。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `args`: 会话 ID 参数
///
/// 返回:
/// - 删除是否成功
fn delete_current_session(paths: &SaiPaths, args: SessionIdArgs) -> Result<()> {
    let deleted = crate::state::delete_session(paths, &args.id)?;
    println!(
        "{}: {}",
        if deleted {
            t("deleted session", "已删除会话")
        } else {
            t("session not found", "未找到会话")
        },
        args.id
    );
    Ok(())
}

/// 重命名会话。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `args`: 重命名参数
///
/// 返回:
/// - 重命名是否成功
fn rename_current_session(paths: &SaiPaths, args: SessionRenameArgs) -> Result<()> {
    let title = join_message(args.title);
    let session = crate::state::rename_session(paths, &args.id, &title)?;
    println!(
        "{}: {}  {}",
        t("renamed session", "已重命名会话"),
        session.id,
        session.title
    );
    Ok(())
}
