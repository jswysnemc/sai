use crate::render::terminal_text as t;
use crate::runtime_recovery::has_visible_runtime_recovery;
use crate::state::failure_recovery::summary::{format_recovery_snapshot, has_visible_recovery};
use crate::state::SessionSnapshot;
use anyhow::Result;

/// 打印命令模式会话结束摘要。
///
/// 参数:
/// - `snapshot`: 当前会话状态快照
///
/// 返回:
/// - 打印是否成功
pub fn print_session_summary(snapshot: &SessionSnapshot) -> Result<()> {
    println!("{}", render_session_summary(snapshot));
    Ok(())
}

/// 渲染命令模式会话结束摘要。
///
/// 参数:
/// - `snapshot`: 当前会话状态快照
///
/// 返回:
/// - 上下文占用与本轮耗时摘要（不含会话 ID）
pub fn render_session_summary(snapshot: &SessionSnapshot) -> String {
    observe_non_display_fields(snapshot);
    // 行首引导点与助手正文共用同一符号，摘要因此落在同一条视觉引导线上
    let mut output = format!(
        "\x1b[2m•\x1b[0m {}: {} / {} {} ({:.1}%)",
        t("Context", "上下文"),
        format_k(snapshot.context_prompt_tokens),
        format_k(snapshot.context_window_tokens),
        t("tokens", "token"),
        snapshot.context_token_ratio * 100.0,
    );
    if snapshot.last_turn_duration_ms > 0 {
        output.push_str(&format!(
            " · {} {}",
            t("Turn", "本轮"),
            format_turn_duration_ms(snapshot.last_turn_duration_ms),
        ));
    }
    if let Some(usage) = snapshot.usage.last_conversation_usage.as_ref() {
        output.push_str(&format!(
            " · \u{f090} {} · \u{f08b} {} · {} {:.1}%",
            format_k_u64(usage.prompt_tokens),
            format_k_u64(usage.completion_tokens),
            t("cache", "缓存"),
            turn_cache_hit_ratio(usage) * 100.0,
        ));
    }
    if snapshot.checkpoint_count > 0 {
        let reason = match snapshot.latest_checkpoint_reason.as_deref() {
            Some("manual") => t("manual", "手动"),
            Some("legacy") => t("legacy migration", "旧记录迁移"),
            _ => t("automatic", "自动"),
        };
        output.push_str(&format!(
            " · {}: {} {} / {} checkpoint ({reason})",
            t("Compaction", "压缩"),
            snapshot.checkpoint_covered_turns,
            t("turns", "轮"),
            snapshot.checkpoint_count,
        ));
    }
    if snapshot.checkpoint_count >= 2 {
        output.push_str(&format!(
            "\n  {}",
            t(
                "This thread has been compacted multiple times; start a new focused thread if details become inaccurate.",
                "当前会话已经多次压缩；如果细节开始失真，请新建聚焦会话继续。"
            )
        ));
    }
    output
}

/// 【终端】【会话摘要】将毫秒格式化为人类可读本轮耗时。
///
/// 参数:
/// - `ms`: 耗时毫秒
///
/// 返回:
/// - 如 `12s` / `12秒` / `1m05s`
pub(crate) fn format_turn_duration_ms(ms: u64) -> String {
    use crate::i18n::is_zh;
    let total_secs = ms / 1_000;
    if is_zh() {
        if total_secs < 60 {
            return format!("{total_secs}秒");
        }
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        if mins < 60 {
            return format!("{mins}分{secs:02}秒");
        }
        let hours = mins / 60;
        let remain_mins = mins % 60;
        return format!("{hours}小时{remain_mins}分{secs:02}秒");
    }
    if total_secs < 60 {
        return format!("{total_secs}s");
    }
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    if mins < 60 {
        return format!("{mins}m{secs:02}s");
    }
    let hours = mins / 60;
    let remain_mins = mins % 60;
    format!("{hours}h{remain_mins:02}m{secs:02}s")
}

/// 读取快照中当前不展示的诊断字段。
///
/// 参数:
/// - `snapshot`: 当前会话状态快照
///
/// 返回:
/// - 无
fn observe_non_display_fields(snapshot: &SessionSnapshot) {
    let _ = (
        snapshot.session_id.as_str(),
        snapshot.turn_count,
        snapshot.context_chars,
        snapshot.context_limit_chars,
        snapshot.context_ratio,
        snapshot.context_prompt_tokens,
        snapshot.context_window_tokens,
        snapshot.context_token_ratio,
        snapshot.checkpoint_count,
        snapshot.checkpoint_covered_turns,
        snapshot.tail_turns,
        snapshot.latest_checkpoint_at.as_deref(),
        snapshot.latest_checkpoint_reason.as_deref(),
        snapshot.usage.requests,
        snapshot.usage.prompt_tokens,
        snapshot.usage.completion_tokens,
        snapshot.usage.total_tokens,
        snapshot
            .usage
            .last_usage
            .as_ref()
            .map(|usage| usage.total_tokens),
        snapshot
            .compaction
            .as_ref()
            .map(|summary| summary.compacted_turns),
        snapshot
            .context_epoch
            .as_ref()
            .map(|epoch| epoch.source_count),
        snapshot
            .session_memory
            .as_ref()
            .map(|memory| memory.source_turn_count),
        snapshot.tool_history.call_count,
        snapshot.dynamic_sources.len(),
        snapshot.projection_warnings.len(),
        snapshot.last_turn_duration_ms,
    );
    if let Some(active_run) = &snapshot.active_run {
        let _ = (
            active_run.owner.as_str(),
            active_run.pid,
            active_run.started_at.as_str(),
            active_run.lock_path.as_str(),
        );
    }
    if has_visible_recovery(&snapshot.recovery) {
        let _ = format_recovery_snapshot(&snapshot.recovery);
    }
    let _ = has_visible_runtime_recovery(&snapshot.runtime_recovery);
}

/// 格式化千单位数值。
///
/// 参数:
/// - `value`: 原始数值
///
/// 返回:
/// - `xxk` 风格文本
fn format_k(value: usize) -> String {
    if value >= 1_000 {
        let scaled = value as f64 / 1_000.0;
        if scaled >= 10.0 {
            format!("{scaled:.0}k")
        } else {
            format!("{scaled:.1}k")
        }
    } else {
        value.to_string()
    }
}

/// 格式化单轮 token 数。
///
/// 参数:
/// - `value`: provider 上报的 token 数
///
/// 返回:
/// - 紧凑千单位文本
fn format_k_u64(value: u64) -> String {
    usize::try_from(value)
        .map(format_k)
        .unwrap_or_else(|_| value.to_string())
}

/// 计算单轮输入缓存命中占比。
///
/// 参数:
/// - `usage`: 同一轮全部模型请求的汇总用量
///
/// 返回:
/// - 0 到 1 之间的缓存读取占比
fn turn_cache_hit_ratio(usage: &crate::llm::Usage) -> f64 {
    if usage.prompt_tokens == 0 {
        return 0.0;
    }
    (usage.cache_read_tokens.min(usage.prompt_tokens) as f64 / usage.prompt_tokens as f64)
        .clamp(0.0, 1.0)
}
