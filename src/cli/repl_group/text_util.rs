//! 群组终端公共排版工具：ANSI 裁剪填充、语义标签、双列对齐、目录浓缩与原生活动圆环。

use crate::cli::repl_chrome::is_light_background;
use crate::cli::repl_text::visible_width;
use crate::render::ansi_clip::clip_ansi_to_width;
use crate::render::panel_palette::{bg, fg, PanelPalette, Rgb};
use crate::render::terminal_palette::terminal_palette;
use crate::super_agent::model::{MemberStatus, TaskStatus};
use chrono::{Local, TimeZone};

pub(super) const RESET: &str = "\x1b[0m";
pub(super) const BOLD: &str = "\x1b[1m";
pub(super) const DIM: &str = "\x1b[2m";

/// 暗色终端顶栏默认背景（与主全屏画布及原生输入面板同档深灰）。
const DARK_HEADER_BG: &str = "\x1b[48;5;235m";

/// 【群组终端】【顶栏底色】按当前终端明暗返回与主全屏视图一致的顶栏背景序列。
///
/// 参数:
/// - `palette`: 当前明暗调色板
///
/// 返回:
/// - 顶栏背景 ANSI 序列
pub(super) fn header_bg(palette: &PanelPalette) -> String {
    if is_light_background(terminal_palette().background) {
        bg(palette.hover_bg)
    } else {
        DARK_HEADER_BG.to_string()
    }
}

/// 【群组终端】【宽度裁剪】安全裁剪含 ANSI 序列的单行文本，超出列宽时加省略号并闭合样式。
///
/// 参数:
/// - `text`: 原始文本
/// - `width`: 最大可视列数
///
/// 返回:
/// - 不超过 `width` 列的文本
pub(super) fn clip_ansi(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if visible_width(text) <= width {
        return text.to_string();
    }
    clip_ansi_to_width(text, width, "…")
}

/// 【群组终端】【ANSI 填充】按显示宽度裁剪或右补空格，确保多栏拼接零错位。
///
/// 参数:
/// - `text`: 可能包含 ANSI 样式的文本
/// - `width`: 目标列宽
///
/// 返回:
/// - 可视宽度严格等于 `width` 的文本（尾部重置样式）
pub(super) fn pad_ansi(text: &str, width: usize) -> String {
    let w = visible_width(text);
    if w > width {
        let clipped = clip_ansi(text, width);
        let cw = visible_width(&clipped);
        format!("{clipped}{RESET}{}", " ".repeat(width.saturating_sub(cw)))
    } else if w < width {
        format!("{text}{RESET}{}", " ".repeat(width - w))
    } else {
        format!("{text}{RESET}")
    }
}

/// 【群组终端】【纯前景标签】对齐主侧栏 `sidebar_widgets::badge`，仅用前景色而不铺重底色。
///
/// 参数:
/// - `label`: 标签文字
/// - `color`: 语义前景色
///
/// 返回:
/// - 带前景色的紧凑标签
pub(super) fn badge(label: &str, color: Rgb) -> String {
    format!("{}{label}\x1b[39m", fg(color))
}

/// 【群组终端】【双列对齐】对齐主侧栏 `sidebar_widgets::columns`，右侧统计优先保留，中间留白补齐。
///
/// 参数:
/// - `left`: 左侧内容
/// - `right`: 右侧内容
/// - `width`: 可用总列宽
///
/// 返回:
/// - 恰好占用 `width` 列的双列文本
pub(super) fn columns(left: &str, right: &str, width: usize) -> String {
    let right = clip_ansi(right, width);
    let right_w = visible_width(&right);
    let left_budget = width.saturating_sub(right_w + usize::from(right_w > 0));
    let left = clip_ansi(left, left_budget);
    let left_w = visible_width(&left);
    format!(
        "{left}{}{right}",
        " ".repeat(width.saturating_sub(left_w + right_w))
    )
}

/// 【群组终端】【分级标题】对齐主侧栏 `heading_with_tone`，以强调色三角、粗体标题与右侧计数建立层级，无需横线分割。
///
/// 参数:
/// - `title`: 分组标题
/// - `count`: 右侧计数文本
/// - `quiet`: 空态降噪标志
/// - `width`: 可用净宽
/// - `palette`: 当前明暗调色板
///
/// 返回:
/// - 定宽分组标题行
pub(super) fn section_heading(
    title: &str,
    count: &str,
    quiet: bool,
    width: usize,
    palette: &PanelPalette,
) -> String {
    let left = if quiet {
        format!("{}· {}{title}\x1b[39m", fg(palette.subtle), fg(palette.muted))
    } else {
        format!(
            "{}▾ {}{BOLD}{title}\x1b[22m\x1b[39m",
            fg(palette.accent),
            fg(palette.text)
        )
    };
    let right = format!(
        "{}{count}\x1b[39m",
        fg(if quiet { palette.subtle } else { palette.muted })
    );
    format!("{}{RESET}", columns(&left, &right, width))
}

/// 【群组终端】【目录浓缩】保留末尾仓库名，超窄预算时以 `…/leaf` 回退。
///
/// 参数:
/// - `path`: 已压缩家目录的路径
/// - `budget`: 可用列宽
///
/// 返回:
/// - 适配宽度的目录文本
pub(super) fn compact_directory(path: &str, budget: usize) -> String {
    if path.is_empty() || budget < 2 {
        return String::new();
    }
    if visible_width(path) <= budget {
        return path.to_string();
    }
    let trimmed = path.trim_end_matches(['/', '\\']);
    let leaf = trimmed
        .rsplit(['/', '\\'])
        .find(|segment| !segment.is_empty() && *segment != "~")
        .unwrap_or(trimmed);
    if leaf.is_empty() {
        return clip_ansi(path, budget);
    }
    let prefixed = format!("…/{leaf}");
    if prefixed != path && visible_width(&prefixed) <= budget {
        return prefixed;
    }
    if visible_width(leaf) <= budget {
        return leaf.to_string();
    }
    clip_ansi(leaf, budget)
}

/// 【群组终端】【路径分层着色】父路径使用次要色，末段项目目录名加粗提亮。
///
/// 参数:
/// - `short_dir`: 浓缩后的目录路径
/// - `palette`: 当前明暗调色板
///
/// 返回:
/// - 分层着色路径文本
pub(super) fn style_directory(short_dir: &str, palette: &PanelPalette) -> String {
    if short_dir.is_empty() {
        return format!("{}{BOLD}sai\x1b[22m", fg(palette.text));
    }
    let trimmed = short_dir.trim_end_matches(['/', '\\']);
    if let Some(pos) = trimmed.rfind(['/', '\\']) {
        let (parent, leaf) = trimmed.split_at(pos + 1);
        if !leaf.is_empty() {
            return format!(
                "{}{parent}{}{BOLD}{leaf}\x1b[22m",
                fg(palette.muted),
                fg(palette.text)
            );
        }
    }
    format!("{}{BOLD}{short_dir}\x1b[22m", fg(palette.text))
}

/// 【群组终端】【圆角顶边】绘制对齐主面板风格的圆角顶边框线 `╭─ ◆ title ───╮`。
///
/// 参数:
/// - `title`: 顶边标题
/// - `width`: 外框总列宽
/// - `palette`: 当前明暗调色板
///
/// 返回:
/// - 恰好占用 `width` 列的顶边线
pub(super) fn rounded_top_border(title: &str, width: usize, palette: &PanelPalette) -> String {
    if width < 4 {
        return "─".repeat(width);
    }
    let inner = width.saturating_sub(2);
    if title.is_empty() {
        return format!("{}╭{}╮{RESET}", fg(palette.border), "─".repeat(inner));
    }
    let max_title = inner.saturating_sub(6);
    let clipped = clip_ansi(title, max_title);
    let title_w = visible_width(&clipped) + 2;
    let fill = inner.saturating_sub(title_w + 3);
    format!(
        "{}╭─ {}◆ {}{BOLD}{clipped}{RESET}{} {}╮{RESET}",
        fg(palette.border),
        fg(palette.accent),
        fg(palette.text),
        fg(palette.border),
        "─".repeat(fill),
    )
}

/// 【群组终端】【圆角底边】绘制圆角底边框线 `╰──────╯`。
///
/// 参数:
/// - `width`: 外框总列宽
/// - `palette`: 当前明暗调色板
///
/// 返回:
/// - 恰好占用 `width` 列的底边线
pub(super) fn rounded_bottom_border(width: usize, palette: &PanelPalette) -> String {
    if width < 2 {
        return "─".repeat(width);
    }
    format!("{}╰{}╯{RESET}", fg(palette.border), "─".repeat(width - 2))
}

/// 判断字符是否属于中英文避头标点（不允许孤立出现在折行后的新行行首）。
fn is_line_start_forbidden(ch: char) -> bool {
    matches!(
        ch,
        '，' | '。'
            | '、'
            | '；'
            | '：'
            | '！'
            | '？'
            | '）'
            | '》'
            | '】'
            | '」'
            | '』'
            | '”'
            | '’'
            | ')'
            | ']'
            | '}'
            | ','
            | '.'
            | ';'
            | ':'
            | '!'
            | '?'
    )
}

/// 【群组终端】【智能折行】按终端显示宽度折行纯文本，优先在 ASCII 单词边界断行并遵循中文避头标点规则。
///
/// 参数:
/// - `text`: 原始纯文本
/// - `max_width`: 每行最大显示宽度
///
/// 返回:
/// - 折行后的字符串列表（每行显示宽度均不超过 `max_width.max(6)`）
pub(super) fn wrap_plain(text: &str, max_width: usize) -> Vec<String> {
    let limit = max_width.max(6);
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_w = 0usize;
    let chars: Vec<char> = text.chars().collect();
    let mut idx = 0usize;

    while idx < chars.len() {
        // 1. 将连续 ASCII 字母/数字/路径符号视为完整词团，超宽时再回退按字符切分
        if chars[idx].is_ascii_alphanumeric() || matches!(chars[idx], '_' | '-' | '/' | '.') {
            let start = idx;
            let mut word = String::new();
            let mut word_w = 0usize;
            while idx < chars.len()
                && (chars[idx].is_ascii_alphanumeric()
                    || matches!(chars[idx], '_' | '-' | '/' | '.'))
            {
                word.push(chars[idx]);
                word_w += 1;
                idx += 1;
            }
            if word_w <= limit {
                if current_w + word_w > limit && !current.is_empty() {
                    lines.push(current.trim_end().to_string());
                    current.clear();
                    current_w = 0;
                }
                current.push_str(&word);
                current_w += word_w;
                continue;
            }
            idx = start;
        }

        let ch = chars[idx];
        idx += 1;
        let ch_w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
        if current_w + ch_w > limit && !current.is_empty() {
            // 2. 即将落在行首的是避头标点时，将当前行末尾整个 ASCII 词团或单个宽字符顺延至下一行与之结伴
            if is_line_start_forbidden(ch) {
                let mut carried = String::new();
                let mut carried_w = 0usize;
                let trimmed = current.trim_end();
                if let Some(last_ch) = trimmed.chars().next_back() {
                    let tail_start = if last_ch.is_ascii_alphanumeric()
                        || matches!(last_ch, '_' | '-' | '/' | '.')
                    {
                        let mut start = trimmed.len();
                        for (pos, c) in trimmed.char_indices().rev() {
                            if c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '/' | '.') {
                                start = pos;
                            } else {
                                break;
                            }
                        }
                        start
                    } else {
                        trimmed.len() - last_ch.len_utf8()
                    };
                    let tail = &trimmed[tail_start..];
                    let tail_w = visible_width(tail);
                    if !last_ch.is_whitespace() && tail_start > 0 && tail_w + ch_w <= limit {
                        lines.push(trimmed[..tail_start].trim_end().to_string());
                        carried.push_str(tail);
                        carried_w = tail_w;
                    } else {
                        lines.push(trimmed.to_string());
                    }
                } else {
                    lines.push(trimmed.to_string());
                }
                current = carried;
                current_w = carried_w;
            } else {
                lines.push(current.trim_end().to_string());
                current.clear();
                current_w = 0;
                if ch == ' ' {
                    continue;
                }
            }
        }
        current.push(ch);
        current_w += ch_w;
    }
    if !current.is_empty() {
        lines.push(current.trim_end().to_string());
    }
    lines
}

/// 【群组终端】【成员状态文案】将成员状态映射为统一中文短标签，避免侧栏与补全中英文混杂。
///
/// 参数:
/// - `status`: 成员运行态枚举
///
/// 返回:
/// - 中文状态标签
pub(super) fn member_status_label(status: MemberStatus) -> &'static str {
    match status {
        MemberStatus::Idle => "空闲",
        MemberStatus::Running => "运行中",
        MemberStatus::Waiting => "等待中",
        MemberStatus::Blocked => "阻塞",
        MemberStatus::Completed => "已完成",
        MemberStatus::Failed => "异常",
        MemberStatus::Stopped => "已停止",
    }
}

/// 【群组终端】【任务状态文案】将任务状态映射为统一中文短标签。
///
/// 参数:
/// - `status`: 任务状态枚举
///
/// 返回:
/// - 中文状态标签
pub(super) fn task_status_label(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "待执行",
        TaskStatus::Running => "进行中",
        TaskStatus::WaitingReview => "待归纳",
        TaskStatus::Delivered => "已交付",
        TaskStatus::Blocked => "阻塞",
        TaskStatus::Failed => "失败",
        TaskStatus::Stopped => "已停止",
    }
}

/// 【群组终端】【预览文本清洗】去除 Markdown 标题井号与多余空白，便于在单行回执与摘要中干净展示。
///
/// 参数:
/// - `raw`: 原始摘要或交付预览文本
///
/// 返回:
/// - 清洗后的单行纯文本
pub(super) fn clean_preview_text(raw: &str) -> String {
    let normalized = raw.replace(['\n', '\r', '\u{3000}'], " ");
    let trimmed = normalized
        .trim()
        .trim_start_matches('#')
        .trim_start_matches([' ', ':', '：', '-', '*']);
    trimmed.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 【群组终端】【时间戳】将毫秒级 Unix 时间戳格式化为本地 `HH:MM`。
///
/// 参数:
/// - `millis`: 毫秒时间戳
///
/// 返回:
/// - `HH:MM` 格式字符串
pub(super) fn format_hhmm(millis: i64) -> String {
    Local
        .timestamp_millis_opt(millis)
        .single()
        .map(|dt| dt.format("%H:%M").to_string())
        .unwrap_or_else(|| "--:--".to_string())
}

/// 【群组终端】【细滚动条】对齐主全屏画布风格：无需滚动时留空，溢出时绘制细轨与滑块。
///
/// 参数:
/// - `height`: 可视行数
/// - `total`: 总行数
/// - `start`: 当前视窗顶部行索引
/// - `palette`: 当前明暗调色板
///
/// 返回:
/// - 每行对应的 1 列 ANSI 滚动条字符
pub(super) fn scrollbar_column(
    height: usize,
    total: usize,
    start: usize,
    palette: &PanelPalette,
) -> Vec<String> {
    if height == 0 {
        return Vec::new();
    }
    if total <= height {
        return vec![" ".to_string(); height];
    }
    let thumb_len = (height * height / total).max(1).min(height);
    let max_start = total.saturating_sub(height);
    let thumb_top = (height.saturating_sub(thumb_len) * start.min(max_start))
        .checked_div(max_start)
        .unwrap_or(0);
    (0..height)
        .map(|row| {
            if row >= thumb_top && row < thumb_top + thumb_len {
                format!("{}█{RESET}", fg(palette.muted))
            } else {
                format!("{}│{RESET}", fg(palette.border))
            }
        })
        .collect()
}

/// 【群组终端】【运行指示器】复用 Sai 原生单列圆环动效 `◜ ◝ ◞ ◟`，保持全站视觉统一。
///
/// 参数:
/// - `tick`: 帧序号
///
/// 返回:
/// - 主题强调色单列圆环文本
pub(super) fn activity_spinner(tick: usize) -> String {
    crate::render::activity_indicator::render_activity_indicator(tick)
}

/// 【群组终端】【紧凑 token 数】将 token 数量格式化为 `1.2k` / `200k` / `1M` 风格。
///
/// 参数:
/// - `value`: token 数量
///
/// 返回:
/// - 缩写文本
pub(super) fn compact_tokens(value: usize) -> String {
    if value >= 1_000_000 {
        let scaled = format!("{:.1}", value as f64 / 1_000_000.0);
        format!("{}M", scaled.trim_end_matches('0').trim_end_matches('.'))
    } else if value >= 1_000 {
        let scaled = format!("{:.1}", value as f64 / 1_000.0);
        format!("{}k", scaled.trim_end_matches('0').trim_end_matches('.'))
    } else {
        value.to_string()
    }
}
