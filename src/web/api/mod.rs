mod agent_options;
mod agents;
mod background_tasks;
mod config;
mod cron_jobs;
mod engine_connection;
mod gateway_weixin_login;
mod gateways;
mod goals;
mod health;
mod input_history;
mod mcp_config;
mod memory;
mod permissions;
mod prompts;
mod providers;
mod questions;
mod runs;
mod session_data;
mod session_runtime;
mod session_tree;
mod sessions;
mod skills;
mod subagents;
mod system;
mod terminal;
mod todos;
mod usage_stats;
mod workspace;
mod workspace_git;
mod workspace_git_events;
mod workspaces;
use super::app_state::WebAppState;
use super::auth;
use axum::middleware;
use axum::routing::{get, post};
use axum::Router;

/// 组装公开与受保护 API 路由。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - API 路由
pub(super) fn router(state: WebAppState) -> Router<WebAppState> {
    let protected = Router::new()
        .merge(workspaces::routes())
        .merge(config::routes())
        .merge(engine_connection::routes())
        .merge(input_history::routes())
        .merge(mcp_config::routes())
        .merge(agent_options::routes())
        .merge(agents::routes())
        .merge(skills::routes())
        .merge(background_tasks::routes())
        .merge(todos::routes())
        .merge(memory::routes())
        .merge(subagents::routes())
        .merge(cron_jobs::routes())
        .merge(providers::routes())
        .merge(prompts::routes())
        .merge(permissions::routes())
        .merge(questions::routes())
        .merge(gateways::routes())
        .merge(goals::routes())
        .merge(gateway_weixin_login::routes())
        .merge(session_tree::routes())
        .merge(session_data::routes())
        .merge(sessions::routes())
        .merge(runs::routes())
        .merge(workspace::routes())
        .merge(workspace_git::routes())
        .merge(workspace_git_events::routes())
        .merge(system::routes())
        .merge(usage_stats::routes())
        .merge(terminal::routes())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_auth,
        ));
    Router::new()
        .route("/api/health", get(health::health))
        .route("/api/auth/session", post(auth::create_session))
        .merge(protected)
}
