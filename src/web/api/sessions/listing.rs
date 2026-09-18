use super::*;

/// 返回按工作区分组的全部会话。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - 工作区及其会话树
pub(super) async fn tree(
    State(state): State<WebAppState>,
) -> WebResult<Json<Vec<WorkspaceSessionsResponse>>> {
    let active_workspace = state.workspaces.active().map_err(WebError::from)?;
    let workspaces = state.workspaces.list().map_err(WebError::from)?;
    let mut result = Vec::with_capacity(workspaces.len());
    for workspace in workspaces {
        let path = FilePath::new(&workspace.path);
        let is_git_repository = crate::web::workspace::is_git_repository(path).await;
        let sessions = crate::state::list_located_sessions_for_workspace(&state.paths, path)
            .map_err(WebError::from)?
            .into_iter()
            .map(|session| {
                let selected = workspace.id == active_workspace.id && session.is_current;
                located_session_response(session, selected)
            })
            .collect();
        result.push(WorkspaceSessionsResponse {
            active: workspace.id == active_workspace.id,
            workspace_id: workspace.id,
            workspace_name: workspace.name,
            workspace_path: workspace.path,
            is_git_repository,
            sessions,
        });
    }
    Ok(Json(result))
}

/// 列出当前工作区会话。
pub(super) async fn list(
    State(state): State<WebAppState>,
) -> WebResult<Json<Vec<SessionResponse>>> {
    let workspace = state.workspaces.active().map_err(WebError::from)?;
    let workspace_path = FilePath::new(&workspace.path);
    let sessions = crate::state::list_located_sessions_for_workspace(&state.paths, workspace_path)
        .map_err(WebError::from)?;
    Ok(Json(
        sessions
            .into_iter()
            .map(|session| {
                let selected = session.is_current;
                located_session_response(session, selected)
            })
            .collect(),
    ))
}

/// 组装会话 API 响应：选中指针与终端/网页加载态分开。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `workspace_path`: 会话所属工作区路径
/// - `session`: 会话索引记录
/// - `selected`: 是否为该工作区当前选中会话
///
/// 返回:
/// - 含加载态的会话响应
pub(super) fn session_response(
    paths: &crate::paths::SaiPaths,
    workspace_path: &FilePath,
    session: crate::state::SessionInfo,
    selected: bool,
) -> SessionResponse {
    let (_, holder) = session_loaded_holder(paths, workspace_path, &session.id);
    response_with_holder(session, selected, holder)
}

/// 读取已打开会话的实例类型，仅用于列表展示。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `workspace_path`: 会话所属工作区路径
/// - `session_id`: 会话标识
///
/// 返回:
/// - `(是否已加载, 实例类型)`
pub(super) fn session_loaded_holder(
    paths: &crate::paths::SaiPaths,
    workspace_path: &FilePath,
    session_id: &str,
) -> (bool, Option<String>) {
    let Ok((_, state_dir)) =
        crate::state::state_dir_for_workspace_session(paths, workspace_path, session_id)
    else {
        return (false, None);
    };
    let holder = loaded_holder(&state_dir);
    (holder.is_some(), holder)
}

/// 【会话载入】【列表响应】使用已定位目录获取在线实例，不逐行读取工作区索引。
/// @param session 含已知目录的索引记录；selected 为界面选中状态
/// @returns 可直接展示的会话响应
fn located_session_response(
    session: crate::state::LocatedSession,
    selected: bool,
) -> SessionResponse {
    let holder = loaded_holder(&session.state_dir);
    response_with_holder(session.info, selected, holder)
}

/// 【会话载入】【在线实例】只读取指定会话的非独占记录。
/// @param state_dir 索引确认的会话数据目录
/// @returns 在线实例类型；无实例或进程已退出时为空
fn loaded_holder(state_dir: &FilePath) -> Option<String> {
    crate::runner::session_instances(state_dir)
        .into_iter()
        .next()
        .map(|record| record.owner)
}

/// 【会话载入】【响应组装】复用同一份索引与在线记录构造列表或管理操作结果。
/// @param session 会话元数据；selected 为选中状态；holder 为兼容字段，仅表示第一个在线实例的类型
/// @returns 会话响应
fn response_with_holder(
    session: crate::state::SessionInfo,
    selected: bool,
    holder: Option<String>,
) -> SessionResponse {
    SessionResponse {
        id: session.id,
        title: session.title,
        created_at: session.created_at,
        updated_at: session.updated_at,
        active: selected,
        loaded: holder.is_some(),
        holder,
    }
}
