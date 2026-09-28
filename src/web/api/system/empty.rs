use super::*;

/// 【系统用量】【空会话】返回零会话用量和实际进程指标，不创建持久化会话。
/// @param state 应用状态；config 提供上下文策略；context_window_tokens 为模型容量
/// @returns 保持原有响应契约的空会话快照
pub(super) fn usage(
    state: &WebAppState,
    config: &AppConfig,
    context_window_tokens: usize,
) -> WebResult<Json<SystemUsageResponse>> {
    let process = state.system_monitor.snapshot();
    let policy = crate::state::CompactionBudgetPolicy::from_context(
        config.context.clamped_compaction_ratio(),
        config.context.compaction_reserve_tokens,
    );
    Ok(Json(SystemUsageResponse {
        session: SessionUsageResponse {
            context_window_tokens,
            compaction_ratio: policy.ratio,
            compaction_reserve_tokens: policy.reserve_tokens,
            compaction_trigger_tokens: policy.trigger_chars(context_window_tokens.max(1)),
            ..Default::default()
        },
        process: ProcessUsageResponse {
            pid: process.pid,
            uptime_seconds: process.uptime_seconds,
            rss_bytes: process.rss_bytes,
            cpu_percent: process.cpu_percent,
        },
        runtime: RuntimeUsageResponse {
            active_run: false,
            terminal_count: state.terminals.list().map_err(WebError::from)?.len(),
        },
    }))
}
