use crate::cli::repl_text::visible_width;
use crate::i18n::text as t;
use crate::render::activity_animation::render_activity_guide_with_color;
use crate::render::session_summary::format_k;
use crate::render::transcript::SubagentOverviewEntry;

/// 左栏固定显示状态，模型用量作为标题末尾的次要信息
const STATUS_COL_MIN: usize = 6;
/// 动效栏与标题栏之间的固定间隔
const COL_GAP: &str = "  ";
/// 运行中引导点色相
const RUNNING_GUIDE: (u8, u8, u8) = (204, 167, 0);
/// 待命引导点色相
const IDLE_GUIDE: (u8, u8, u8) = (97, 175, 239);

/// 渲染单行选择条目：选择箭头与标题着色，左栏动效保持原色。
///
/// 参数:
/// - `selected`: 是否为当前高亮
/// - `left`: 已对齐的 tokens / 状态栏（可含扫光 ANSI）
/// - `title`: 右栏标题
///
/// 返回:
/// - ANSI 行
pub(super) fn selection_line(selected: bool, left: &str, title: &str) -> String {
    let marker = if selected {
        "\x1b[1m\x1b[36m  ❯ \x1b[0m"
    } else {
        "    "
    };
    let title = if selected {
        format!("\x1b[1m\x1b[36m{title}\x1b[0m")
    } else {
        format!("\x1b[2m{title}\x1b[0m")
    };
    format!("{marker}{left}{COL_GAP}{title}")
}

/// 组装条目右栏：名称 · 步数 · 时长。
///
/// 类型作短前缀，阶段细节不再进标题，避免与 todo、队列抢宽度。
///
/// 参数:
/// - `entry`: 子智能体概览条目
///
/// 返回:
/// - 右栏纯文本
pub(super) fn render_entry_title(entry: &SubagentOverviewEntry) -> String {
    let label = clip_chars(entry.label.trim(), 28);
    let identity = match entry
        .agent_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(agent_type) => format!("{} {label}", clip_chars(agent_type, 10)),
        None => label,
    };
    let mut parts = vec![identity];
    if let Some((step, max_steps)) = entry.progress {
        parts.push(format!("{step}/{max_steps}"));
    }
    if let Some(elapsed) = entry.elapsed_seconds.filter(|seconds| *seconds > 0) {
        parts.push(format_elapsed(elapsed));
    }
    if let Some(tokens) = entry.tokens {
        parts.push(format!("{} tokens", format_token_plain(tokens)));
    }
    parts.join(" · ")
}

/// 按字符数截断，超长时以省略号收尾。
///
/// 参数:
/// - `text`: 原始文本
/// - `max_chars`: 最大字符数
///
/// 返回:
/// - 截断后的文本
pub(super) fn clip_chars(text: &str, max_chars: usize) -> String {
    // 按显示列数截断：中文 agent 名按字符数截断会撑到近两倍宽
    crate::render::clip_to_width(text, max_chars, "…")
}

/// 把秒数压成面板可读的短时长。
///
/// 参数:
/// - `seconds`: 运行时长
///
/// 返回:
/// - 形如 42s / 3m07s / 2h05m 的短文本
pub(super) fn format_elapsed(seconds: u64) -> String {
    if seconds < 60 {
        return format!("{seconds}s");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m{:02}s", seconds % 60);
    }
    format!("{}h{:02}m", minutes / 60, minutes % 60)
}

/// 格式化 token 左栏纯文本。
pub(super) fn format_token_plain(tokens: u64) -> String {
    format_k(usize::try_from(tokens).unwrap_or(usize::MAX))
}

/// 非焦点态的单行摘要。
///
/// 与 todo、消息队列共用同一套沉底装饰：一行引导点摘要，避免多智能体
/// 一开就把底栏铺成一块列表，把输入框顶上去。条目细节留给 ↓ 展开。
///
/// 参数:
/// - `entries`: 当前子 agent 概览
/// - `frame`: live 动效帧序号
///
/// 返回:
/// - 单行 ANSI 摘要
pub(super) fn idle_line(entries: &[SubagentOverviewEntry], frame: usize) -> String {
    let running = entries.iter().filter(|entry| entry.running).count();
    let mut summary = format!("{} ({})", t("subagents", "子任务"), entries.len());
    if running > 0 {
        summary.push_str(&format!(" · {} {}", running, t("running", "运行中")));
    }
    let idle = entries
        .iter()
        .filter(|entry| entry.status == "idle")
        .count();
    let failed = entries.iter().filter(|entry| entry.status == "err").count();
    let completed = entries.iter().filter(|entry| entry.status == "ok").count();
    let interrupted = entries
        .iter()
        .filter(|entry| entry.status == "interrupted")
        .count();
    let cancelled = entries
        .iter()
        .filter(|entry| entry.status == "cancelled")
        .count();
    if failed > 0 {
        summary.push_str(&format!(" · {failed} {}", t("need attention", "需处理")));
    }
    if idle > 0 {
        summary.push_str(&format!(" · {idle} {}", t("idle", "待命")));
    }
    if interrupted > 0 {
        summary.push_str(&format!(" · {interrupted} {}", t("interrupted", "已中断")));
    }
    if cancelled > 0 {
        summary.push_str(&format!(" · {cancelled} {}", t("stopped", "已停止")));
    }
    if running == 0
        && failed == 0
        && idle == 0
        && interrupted == 0
        && cancelled == 0
        && completed > 0
    {
        summary.push_str(&format!(" · {completed} {}", t("completed", "已完成")));
    }
    // 引导点之外全部压暗：与 todo、队列同一套沉底装饰，只有状态点是亮色
    format!(
        "{} \x1b[2m{summary}  ▸ ↓ {}\x1b[0m",
        guide_dot(entries, frame),
        t("expand", "展开")
    )
}

/// 沉底引导点：运行中走流光，有待命条目用待命色，其余为静态暗点。
///
/// 参数:
/// - `entries`: 当前子 agent 概览
/// - `frame`: live 动效帧序号
///
/// 返回:
/// - 引导点 ANSI 文本，宽度恒为一列
pub(super) fn guide_dot(entries: &[SubagentOverviewEntry], frame: usize) -> String {
    if entries.iter().any(|entry| entry.running) {
        render_activity_guide_with_color(frame, Some(RUNNING_GUIDE))
    } else if entries.iter().any(|entry| entry.status == "err") {
        "\x1b[31m●\x1b[0m".to_string()
    } else if entries.iter().any(|entry| entry.status == "idle") {
        let (red, green, blue) = IDLE_GUIDE;
        format!("\x1b[38;2;{red};{green};{blue}m○\x1b[0m")
    } else {
        "\x1b[2m●\x1b[0m".to_string()
    }
}

/// 条目左栏始终展示运行状态；参数为条目，返回本地化状态文本。
pub(super) fn entry_left_plain(entry: &SubagentOverviewEntry) -> String {
    match entry.status {
        "run" => t("Running", "运行"),
        "idle" => t("Idle", "待命"),
        "err" => t("Failed", "失败"),
        "cancelled" => t("Stopped", "已停止"),
        "interrupted" => t("Interrupted", "已中断"),
        _ => t("Done", "完成"),
    }
    .to_string()
}

/// 计算两栏布局的左栏宽度，保证数字与标题分别对齐。
pub(super) fn status_column_width(entries: &[SubagentOverviewEntry]) -> usize {
    entries
        .iter()
        .map(|entry| visible_width(&entry_left_plain(entry)) + 2)
        .max()
        .unwrap_or(0)
        .max(STATUS_COL_MIN)
}

/// 按可见列宽左补空格，让数字右对齐。
pub(super) fn pad_left(text: &str, width: usize) -> String {
    let pad = width.saturating_sub(visible_width(text));
    format!("{}{text}", " ".repeat(pad))
}

/// 渲染状态列；参数为条目、列宽和帧号，返回单列动画及可读状态。
pub(super) fn render_entry_left(
    entry: &SubagentOverviewEntry,
    width: usize,
    frame: usize,
) -> String {
    let text = entry_left_plain(entry);
    let marker = if entry.running {
        render_activity_guide_with_color(frame, Some(RUNNING_GUIDE))
    } else {
        match entry.status {
            "err" => "\x1b[31m●\x1b[0m",
            "idle" => "\x1b[36m○\x1b[0m",
            "cancelled" | "interrupted" => "\x1b[2m○\x1b[0m",
            _ => "\x1b[32m●\x1b[0m",
        }
        .to_string()
    };
    let pad = " ".repeat(width.saturating_sub(visible_width(&text) + 2));
    format!("{marker} {text}{pad}")
}
