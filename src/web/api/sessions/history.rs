use super::*;

/// 读取指定会话消息历史。
pub(super) async fn messages(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> WebResult<Json<Vec<crate::state::StoredConversationEntry>>> {
    let store = StateStore::for_session(&state.paths, &id)
        .map_err(|error| WebError::not_found(error.to_string()))?;
    let history = store
        .history(query.limit.unwrap_or(200).clamp(1, 2000))
        .map_err(WebError::from)?;
    Ok(Json(history))
}

/// 读取指定会话的结构化轮次与工具时间线。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `id`: 会话 ID
/// - `query`: 轮次数量限制
///
/// 返回:
/// - 带每轮模型标识的会话时间线
pub(super) async fn timeline(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> WebResult<Json<TimelineResponse>> {
    let store = StateStore::for_session(&state.paths, &id)
        .map_err(|error| WebError::not_found(error.to_string()))?;
    let mut timeline = store
        .session_timeline_with_compaction(query.limit.unwrap_or(200).clamp(1, 2000))
        .map_err(WebError::from)?;
    permission_timeline::attach_permission_decisions(&store, &mut timeline.turns)
        .map_err(WebError::from)?;
    // 附加每轮记录的模型标识，前端据此在模型变化处绘制切换分割线
    let mut models = store.turn_models().map_err(WebError::from)?;
    let turns = timeline
        .turns
        .into_iter()
        .map(|turn| {
            let model = models.remove(&turn.turn_id);
            TimelineTurnResponse { turn, model }
        })
        .collect();
    Ok(Json(TimelineResponse {
        turns,
        compaction: timeline.compaction,
    }))
}
