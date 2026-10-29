use serde_json::Value;

/// 【子任务卡片】【运行统计】读取实际调用次数、用量和任务时间，缺失字段保持不显示。
/// 参数：task 为公开子任务快照；返回：以分隔符开头的紧凑统计文本。
pub(super) fn suffix(task: &Value) -> String {
    let stats = &task["stats"];
    let mut fields = Vec::new();
    if let Some(calls) = stats["tool_calls"]
        .as_u64()
        .or_else(|| task["step"].as_u64())
    {
        fields.push(format!(
            "{calls} {}",
            crate::render::terminal_text("tool calls", "步调用")
        ));
    }
    let total = stats["total_tokens"].as_u64();
    let estimate = stats["token_estimate"].as_u64().filter(|count| *count > 0);
    if let Some(tokens) = estimate.or(total) {
        let estimated = estimate.is_some()
            && (stats["token_estimate_is_actual"] == false
                || stats["token_estimate_method"]
                    .as_str()
                    .is_some_and(|method| method != "provider_usage")
                || tokens > total.unwrap_or(0));
        fields.push(format!(
            "{} tokens",
            crate::tools::format_token_count(tokens, estimated)
        ));
    }
    // 1. 【子任务卡片】【任务耗时】只读终态快照，不把 wait/status 工具的等待时间当作子任务执行时间
    if task["status"]
        .as_str()
        .is_some_and(|status| status != "running")
    {
        if let Some(seconds) = task["started_at"]
            .as_u64()
            .filter(|start| *start > 0)
            .zip(task["updated_at"].as_u64())
            .and_then(|(start, end)| end.checked_sub(start))
        {
            fields.push(format!("{seconds}s"));
        }
    }
    if fields.is_empty() {
        String::new()
    } else {
        format!(" · {}", fields.join(" · "))
    }
}
