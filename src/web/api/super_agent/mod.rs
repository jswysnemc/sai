mod channels;
mod groups;
mod orchestration;

#[cfg(test)]
mod tests;

use super::super::app_state::WebAppState;
use axum::routing::{delete, get, post};
use axum::Router;

/// 【Web API】【Super-Agent 路由】注册群组对话、Runner 编排、渠道接入、Watchdog 与 SSE 实时流接口。
///
/// 参数:
/// - 无
///
/// 返回:
/// - Super-Agent 受保护 API 路由集合
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/super-agent/overview", get(groups::overview))
        .route("/api/super-agent/stream", get(channels::stream))
        .route(
            "/api/super-agent/config",
            get(channels::get_config).put(channels::update_config),
        )
        .route("/api/super-agent/watchdog", get(channels::list_watchdog))
        .route(
            "/api/super-agent/watchdog/check",
            post(channels::run_watchdog_check),
        )
        .route(
            "/api/super-agent/groups",
            get(groups::list_groups).post(groups::create_group),
        )
        .route(
            "/api/super-agent/groups/:id",
            get(groups::group_detail)
                .patch(groups::update_group)
                .delete(groups::delete_group),
        )
        .route(
            "/api/super-agent/groups/:id/messages",
            get(groups::list_messages).post(groups::post_message),
        )
        .route("/api/super-agent/groups/:id/recall", get(groups::recall))
        .route(
            "/api/super-agent/groups/:id/runners",
            post(orchestration::spawn_runner),
        )
        .route(
            "/api/super-agent/groups/:id/runners/:runner_id/message",
            post(orchestration::message_runner),
        )
        .route(
            "/api/super-agent/groups/:id/runners/:runner_id/stop",
            post(orchestration::stop_runner),
        )
        .route(
            "/api/super-agent/groups/:id/runners/:runner_id/subscribe",
            post(orchestration::update_subscription),
        )
        .route(
            "/api/super-agent/groups/:id/tasks",
            get(orchestration::list_tasks),
        )
        .route(
            "/api/super-agent/groups/:id/tasks/:task_id/retry",
            post(orchestration::retry_task),
        )
        .route(
            "/api/super-agent/groups/:id/tasks/:task_id/stop",
            post(orchestration::stop_task),
        )
        .route(
            "/api/super-agent/groups/:id/triggers",
            get(orchestration::list_triggers),
        )
        .route(
            "/api/super-agent/groups/:id/channels",
            get(channels::list_channels).post(channels::bind_channel),
        )
        .route(
            "/api/super-agent/groups/:id/channels/:binding_id",
            delete(channels::unbind_channel),
        )
        .route(
            "/api/super-agent/groups/:id/channel-inbound",
            post(channels::ingest_channel_inbound),
        )
}
