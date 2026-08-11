use crate::render::markdown_blocks::horizontal_rule_width;
use crate::render::table::visible_width;
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
/// 视觉分层：标签与分隔符弱化，数值保持常规亮度并按语义着色——
/// 上下文占比按压力从弱化渐进到黄、红，上行 token 青色、下行绿色，
/// 缓存命中绿色。扫一眼只看到彩色数值，细读才需要标签。
///
/// 参数:
/// - `snapshot`: 当前会话状态快照
///
/// 返回:
/// - 上下文占用与本轮耗时摘要（不含会话 ID）
pub fn render_session_summary(snapshot: &SessionSnapshot) -> String {
    observe_non_display_fields(snapshot);
    // 行首引导点与助手正文共用同一符号，摘要因此落在同一条视觉引导线上
    let ratio = snapshot.context_token_ratio;
    let mut output = format!(
        "\x1b[2m•\x1b[0m \x1b[2m{}:\x1b[0m {} / {} \x1b[2m{}\x1b[0m {}({:.1}%)\x1b[0m",
        t("Context", "上下文"),
        format_k(snapshot.context_prompt_tokens),
        format_k(snapshot.context_window_tokens),
        t("tokens", "token"),
        context_ratio_style(ratio),
        ratio * 100.0,
    );
    if snapshot.last_turn_duration_ms > 0 {
        output.push_str(&format!(
            " \x1b[2m· {}\x1b[0m {}",
            t("Turn", "本轮"),
            format_turn_duration_ms(snapshot.last_turn_duration_ms),
        ));
    }
    if let Some(usage) = snapshot.usage.last_conversation_usage.as_ref() {
        output.push_str(&format!(
            " \x1b[2m·\x1b[0m \x1b[36m↑ {}\x1b[0m \x1b[2m·\x1b[0m \x1b[32m↓ {}\x1b[0m \x1b[2m· {}\x1b[0m \x1b[32m{:.1}%\x1b[0m",
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
    // 总览块末尾水平线分隔 turn：有空位则右接，否则换行全宽（不是竖线 │）
    append_right_turn_rule(&mut output);
    output
}

/// 【终端】【会话分隔】判断文本是否已带 turn 水平分隔线。
///
/// 参数:
/// - `text`: 可能含 ANSI 的总览或 meta 文本
///
/// 返回:
/// - 含纯 `─` 分隔行或同行尾部横线时为 true
pub(crate) fn has_turn_rule(text: &str) -> bool {
    let plain = crate::render::activity_animation::strip_ansi_for_test(text);
    plain.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.len() >= 3 && trimmed.chars().all(|ch| ch == '─') {
            return true;
        }
        line.contains('─')
            && line
                .trim_end()
                .chars()
                .rev()
                .take_while(|ch| *ch == '─')
                .count()
                >= 3
    })
}

/// 【终端】【会话分隔】去掉已烘焙的 turn 横线，便于按当前正文宽度重画。
///
/// 参数:
/// - `text`: 可能含同行或换行横线的总览文本
///
/// 返回:
/// - 不含 turn 横线的正文
pub(crate) fn strip_turn_rule(text: &str) -> String {
    let mut kept = Vec::new();
    for line in text.split('\n') {
        let plain = crate::render::activity_animation::strip_ansi_for_test(line);
        let trimmed = plain.trim();
        if trimmed.len() >= 3 && trimmed.chars().all(|ch| ch == '─') {
            continue;
        }
        kept.push(strip_same_line_turn_rule(line));
    }
    while kept.last().is_some_and(|line| line.is_empty()) {
        kept.pop();
    }
    kept.join("\n")
}

/// 【终端】【会话分隔】按指定列宽重画 turn 横线（始终另起一行，避免与摘要同行后被 TUI 再折行）。
///
/// 参数:
/// - `text`: 原始总览（可已含错误宽度的横线）
/// - `width`: 当前 transcript 正文净宽度
///
/// 返回:
/// - 横线宽度与 `width` 对齐后的文本
pub(crate) fn refit_turn_rule(text: &str, width: usize) -> String {
    if !has_turn_rule(text) && !looks_like_session_summary(text) {
        return text.to_string();
    }
    let mut body = strip_turn_rule(text);
    append_right_turn_rule_with_width(&mut body, width.max(1));
    body
}

/// 【终端】【会话分隔】识别会话总览行（Context / 上下文）。
fn looks_like_session_summary(text: &str) -> bool {
    let plain = crate::render::activity_animation::strip_ansi_for_test(text);
    plain.contains("Context") || plain.contains("上下文")
}

/// 去掉行尾同行右接的 `─` 横线（含弱化 ANSI 包装）。
fn strip_same_line_turn_rule(line: &str) -> String {
    let plain = crate::render::activity_animation::strip_ansi_for_test(line);
    let dash_suffix = plain
        .trim_end()
        .chars()
        .rev()
        .take_while(|ch| *ch == '─')
        .count();
    if dash_suffix < 3 {
        return line.to_string();
    }
    let prefix_plain = plain.trim_end().trim_end_matches('─').trim_end();
    truncate_ansi_to_plain_prefix(line, prefix_plain)
}

/// 将 ANSI 行截到与纯文本前缀相同的可见内容处。
fn truncate_ansi_to_plain_prefix(line: &str, prefix_plain: &str) -> String {
    if prefix_plain.is_empty() {
        return String::new();
    }
    let mut plain_len = 0usize;
    let mut index = 0usize;
    let target = prefix_plain.chars().count();
    while index < line.len() && plain_len < target {
        let ch = line[index..].chars().next().unwrap_or_default();
        if ch == '\x1b' {
            let end = crate::render::terminal_image::escape_sequence_end(line, index);
            index = end.max(index + ch.len_utf8());
            continue;
        }
        plain_len += 1;
        index += ch.len_utf8();
    }
    line[..index].trim_end().to_string()
}

/// 【终端】【会话分隔】为总览补水平 turn 分隔线。
///
/// 始终另起一行画横线：同行右接会在「整屏宽度烘焙、TUI 按正文净宽折行」时溢出到下一行。
/// 不再使用竖线 `│`。
///
/// 参数:
/// - `output`: 已渲染的总览文本（可含 ANSI）
///
/// 返回:
/// - 无
fn append_right_turn_rule(output: &mut String) {
    append_right_turn_rule_with_width(output, horizontal_rule_width().max(1));
}

/// 按给定列宽追加下一行 turn 横线。
fn append_right_turn_rule_with_width(output: &mut String, cols: usize) {
    let cols = cols.max(1);
    let plain = crate::render::activity_animation::strip_ansi_for_test(output);
    let first = plain.lines().next().unwrap_or("");
    let used = visible_width(first);
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    output.push_str(&format!("\x1b[2m{}\x1b[0m", "─".repeat(cols)));
}

/// 【终端】【会话摘要】按上下文压力选择占比数值的着色。
///
/// 低压力时占比是背景信息，随标签一起弱化；接近上限时逐级升黄、升红，
/// 提醒该压缩或另起会话了。
///
/// 参数:
/// - `ratio`: 0 到 1 之间的上下文占用比例
///
/// 返回:
/// - 占比数值的 ANSI 前景样式
fn context_ratio_style(ratio: f32) -> &'static str {
    if ratio >= 0.85 {
        "\x1b[31m"
    } else if ratio >= 0.6 {
        "\x1b[33m"
    } else {
        "\x1b[2m"
    }
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
