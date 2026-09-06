use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use std::path::Path;

/// 真正持有运行锁的会话，不包含只打开窗口或排队的会话。
#[derive(Debug, Serialize)]
struct RunningSession {
    workspace_id: String,
    session_id: String,
}

/// 【会话导航】【运行状态】注册只读状态接口。
/// 参数：无；返回：会话运行状态路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new().route("/api/sessions/activity", get(activity))
}

/// 【会话导航】【运行状态】查询各工作区持有有效运行锁的会话。
/// 参数：`state` 为应用状态；返回：网页、终端和网关的运行会话列表。
async fn activity(State(state): State<WebAppState>) -> WebResult<Json<Vec<RunningSession>>> {
    let sessions = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<RunningSession>> {
        let mut result = Vec::new();
        // 1. 仅遍历已登记工作区，读取状态目录而不加载会话执行者
        for workspace in state.workspaces.list()? {
            let workspace_path = Path::new(&workspace.path);
            for session in crate::state::list_sessions_for_workspace(&state.paths, workspace_path)?
            {
                let (_, state_dir) = crate::state::state_dir_for_workspace_session(
                    &state.paths,
                    workspace_path,
                    &session.id,
                )?;
                // 2. 复用运行锁判活规则，排除已结束和进程退出后的残留记录
                if session_is_running(&state_dir, &session.id) {
                    result.push(RunningSession {
                        workspace_id: workspace.id.clone(),
                        session_id: session.id,
                    });
                }
            }
        }
        Ok(result)
    })
    .await
    .map_err(anyhow::Error::from)
    .map_err(WebError::from)?
    .map_err(WebError::from)?;
    Ok(Json(sessions))
}

/// 校验运行锁归属及持有进程存活状态。
/// 参数：`state_dir` 为状态目录，`session_id` 为会话标识；返回：是否正在工作。
fn session_is_running(state_dir: &Path, session_id: &str) -> bool {
    crate::runner::active_run(state_dir).is_some_and(|run| run.session_id == session_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{ActiveRunGuard, SessionHolderGuard, SessionOwner};

    /// 验证只打开终端不算运行，轮次完成后立即撤销运行状态。
    /// 参数：无；返回：无。
    #[test]
    fn only_running_turns_show_activity() {
        let root = tempfile::tempdir().unwrap();
        let _holder =
            SessionHolderGuard::acquire(root.path(), "session_activity", SessionOwner::Repl)
                .unwrap();
        assert!(!session_is_running(root.path(), "session_activity"));
        let guard = ActiveRunGuard::acquire_with_state_dir(
            "session_activity",
            SessionOwner::Repl,
            root.path(),
        )
        .unwrap();
        assert!(session_is_running(root.path(), "session_activity"));
        assert!(!session_is_running(root.path(), "another_session"));
        drop(guard);
        assert!(!session_is_running(root.path(), "session_activity"));
    }
}
