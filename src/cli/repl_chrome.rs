use super::repl_text::visible_width;
use super::*;
use crate::config::AppConfig;
use crate::state::StateStore;

/// REPL 底栏与输入框 chrome 状态。
#[derive(Debug, Clone)]
pub(super) struct ReplChrome {
    pub(super) mode: AgentMode,
    pub(super) context_ratio: f32,
    pub(super) context_window_tokens: usize,
    pub(super) model: String,
    pub(super) thinking: String,
    pub(super) directory: String,
}

impl ReplChrome {
    /// 从当前配置与会话状态构造 chrome。
    ///
    /// 参数:
    /// - `config`: 应用配置
    /// - `state`: 会话状态
    /// - `mode`: 当前 Agent 模式
    ///
    /// 返回:
    /// - chrome 状态
    pub(super) fn from_runtime(config: &AppConfig, state: &StateStore, mode: AgentMode) -> Self {
        let context_limit = config.active_context_window_tokens().unwrap_or(128_000);
        let snapshot = state.session_snapshot(context_limit).ok();
        let provider = config.provider(None).ok();
        let model = provider
            .map(|provider| provider.default_model.trim().to_string())
            .filter(|model| !model.is_empty())
            .unwrap_or_else(|| "-".to_string());
        let thinking = provider
            .map(|provider| provider.thinking_level.trim().to_string())
            .filter(|level| !level.is_empty())
            .unwrap_or_else(|| "auto".to_string());
        let directory = crate::runtime_cwd::current_dir()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| "?".to_string());
        Self {
            mode,
            context_ratio: snapshot
                .as_ref()
                .map(|item| item.context_token_ratio)
                .unwrap_or(0.0),
            context_window_tokens: snapshot
                .as_ref()
                .map(|item| item.context_window_tokens)
                .unwrap_or(context_limit),
            model,
            thinking,
            directory,
        }
    }

    /// 更新模式（Shift+Tab 切换时）。
    ///
    /// 参数:
    /// - `mode`: 新模式
    pub(super) fn set_mode(&mut self, mode: AgentMode) {
        self.mode = mode;
    }

    /// 左侧上下文占用文案。
    ///
    /// 返回:
    /// - 如 `0.0%/272k (auto)`
    pub(super) fn context_status(&self) -> String {
        let pct = (self.context_ratio * 100.0).clamp(0.0, 999.9);
        format!("{pct:.1}%/{}", format_token_k(self.context_window_tokens))
    }

    /// 模式纯文本（用于宽度计算）。
    ///
    /// 返回:
    /// - `yolo` / `plan`
    pub(super) fn mode_plain(&self) -> &'static str {
        match self.mode {
            AgentMode::Yolo => "yolo",
            AgentMode::Audited => "audit",
            AgentMode::AutoAudit => "auto-audit",
            AgentMode::Plan => "plan",
        }
    }

    /// 底栏整行：模式、上下文、模型、思考等级、目录和 Git 分支。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    ///
    /// 返回:
    /// - 已着色状态行
    pub(super) fn footer_line(&self, cols: usize) -> String {
        let cols = cols.max(1);
        let left_plain = format!(
            "{}  {}  {}  {}",
            self.mode_plain(),
            self.context_status(),
            self.model,
            self.thinking
        );
        // 1. 先在纯文本上裁剪，保证 left + 空格 + right 不超过终端列数
        //    禁止对 gap 强制 max(1)：贴满时再塞空格会变成 cols+1 并触发终端换行
        let (left_text, right_text, gap) =
            fit_status_segments(&left_plain, &self.directory, cols);
        // 2. 裁剪后再着色，避免 ANSI 干扰宽度计算
        let left = colorize_left_status(self.mode, &left_text, self.context_ratio);
        let right = if right_text.is_empty() {
            String::new()
        } else {
            color_directory(&right_text)
        };
        format!("{left}{}{}", " ".repeat(gap), right)
    }

}

/// 给模型名称使用稳定的重点颜色。
fn color_model(value: &str) -> String {
    format!("\x1b[38;5;81m{value}\x1b[0m")
}

/// 给思考等级使用独立颜色。
fn color_thinking(value: &str) -> String {
    format!("\x1b[38;5;177m{value}\x1b[0m")
}

/// 给右侧当前目录使用弱化但可辨识的颜色。
fn color_directory(value: &str) -> String {
    format!("\x1b[38;5;110m{value}\x1b[0m")
}

/// 将 token 数格式化为 `272k` 风格。
///
/// 参数:
/// - `value`: token 数
///
/// 返回:
/// - 缩写文本
fn format_token_k(value: usize) -> String {
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

/// 将左右状态段裁剪到终端宽度内。
///
/// 参数:
/// - `left`: 左侧纯文本
/// - `right`: 右侧纯文本
/// - `cols`: 终端列数
///
/// 返回:
/// - (左侧文本, 右侧文本, 中间空格数)；总显示宽度不超过 cols
fn fit_status_segments(left: &str, right: &str, cols: usize) -> (String, String, usize) {
    let cols = cols.max(1);
    let mut left = left.to_string();
    let mut right = right.to_string();

    // 1. 右侧先让位：优先保留模式/上下文/模型
    let left_w = visible_width(&left);
    let sep = usize::from(!left.is_empty() && !right.is_empty());
    let right_budget = cols.saturating_sub(left_w.min(cols) + sep);
    if visible_width(&right) > right_budget {
        right = truncate_to_width(&right, right_budget);
    }

    // 2. 若左侧本身过长，再裁左侧
    let right_w = visible_width(&right);
    let sep = usize::from(!left.is_empty() && !right.is_empty());
    let left_budget = cols.saturating_sub(right_w + sep);
    if visible_width(&left) > left_budget {
        left = truncate_to_width(&left, left_budget);
    }

    // 3. gap 可为 0；仅在有剩余列时填充，绝不超过 cols
    let used = visible_width(&left) + visible_width(&right);
    let gap = cols.saturating_sub(used);
    (left, right, gap)
}

/// 将纯文本左侧状态按段落重新着色（mode / context / model / thinking）。
///
/// 参数:
/// - `mode`: 当前模式
/// - `left_text`: 已裁剪的左侧纯文本
/// - `context_ratio`: 上下文占用比例
///
/// 返回:
/// - 着色后的左侧状态
fn colorize_left_status(mode: AgentMode, left_text: &str, context_ratio: f32) -> String {
    // 用双空格切回四段；截断后段数可能不足
    let parts = left_text.split("  ").collect::<Vec<_>>();
    if parts.len() < 2 {
        return color_model(left_text);
    }
    let mode_text = parts[0];
    let context = parts.get(1).copied().unwrap_or("");
    let model = parts.get(2).copied().unwrap_or("");
    let thinking = parts.get(3..).map(|rest| rest.join("  ")).unwrap_or_default();

    let mode_colored = match mode {
        AgentMode::Yolo => format!("\x1b[38;5;208m{mode_text}\x1b[0m"),
        AgentMode::Audited => format!("\x1b[35m{mode_text}\x1b[0m"),
        AgentMode::AutoAudit => format!("\x1b[38;5;141m{mode_text}\x1b[0m"),
        AgentMode::Plan => format!("\x1b[36m{mode_text}\x1b[0m"),
    };
    let context_color = if context_ratio >= 0.9 {
        "\x1b[31m"
    } else if context_ratio >= 0.7 {
        "\x1b[33m"
    } else {
        "\x1b[32m"
    };
    let mut out = format!("{mode_colored}  {context_color}{context}\x1b[0m");
    if !model.is_empty() {
        out.push_str("  ");
        out.push_str(&color_model(model));
    }
    if !thinking.is_empty() {
        out.push_str("  ");
        out.push_str(&color_thinking(&thinking));
    }
    out
}

/// 将纯文本截断到指定显示宽度。
///
/// 参数:
/// - `value`: 原始信息文本（不应含 ANSI）
/// - `width`: 最大显示宽度
///
/// 返回:
/// - 不超过最大宽度的文本
fn truncate_to_width(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if visible_width(value) <= width {
        return value.to_string();
    }
    if width <= 3 {
        return ".".repeat(width);
    }
    let mut output = String::new();
    let mut used = 0usize;
    for ch in value.chars() {
        // 宽字符按 2 列计
        let char_width = if (ch as u32) >= 0x2e80 { 2 } else { 1 };
        if used.saturating_add(char_width) > width - 3 {
            break;
        }
        output.push(ch);
        used = used.saturating_add(char_width);
    }
    output.push_str("...");
    output
}

/// 生成全宽浅色分隔线。
///
/// 参数:
/// - `cols`: 终端列数
///
/// 返回:
/// - 带样式的分隔线
/// 输入框上下分隔线样式。
///
/// 使用柔和蓝色，区别于正文里的 dim 水平线（`\x1b[2m`）与代码块青色边框（`\x1b[36m`）。
const CHROME_RULE_STYLE: &str = "\x1b[38;5;67m";

/// 生成输入框顶/底分隔线。
///
/// 参数:
/// - `cols`: 终端列数
///
/// 返回:
/// - 带颜色的整行分隔线
pub(super) fn chrome_rule(cols: usize) -> String {
    format!("{CHROME_RULE_STYLE}{}\x1b[0m", "─".repeat(cols.max(1)))
}

/// 生成左右对齐的状态行。
///
/// 参数:
/// - `left`: 左侧文本（无样式）
/// - `right`: 右侧文本（无样式）
/// - `cols`: 终端列数
///
/// 返回:
/// - 带 dim 样式的状态行
#[cfg(test)]
pub(super) fn chrome_status_line(left: &str, right: &str, cols: usize) -> String {
    let cols = cols.max(1);
    let left_w = visible_width(left);
    let right_w = visible_width(right);
    if left_w + right_w + 1 >= cols {
        return format!("\x1b[2m{left} {right}\x1b[0m");
    }
    let gap = cols.saturating_sub(left_w + right_w);
    format!("\x1b[2m{left}{}{right}\x1b[0m", " ".repeat(gap))
}

/// 极简 chrome 固定占用行数：顶线 + 底线 + 状态（模式并入状态行左侧）。
///
/// 返回:
/// - 固定行数
pub(super) fn chrome_fixed_rows() -> u16 {
    3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_line_keeps_left_and_right() {
        let line = chrome_status_line("0.0%/272k (auto)", "gpt · xhigh", 40);
        assert!(line.contains("0.0%/272k (auto)"));
        assert!(line.contains("gpt · xhigh"));
    }

    #[test]
    fn footer_puts_mode_before_context() {
        let chrome = ReplChrome {
            mode: AgentMode::Yolo,
            context_ratio: 0.0,
            context_window_tokens: 272_000,
            model: "gpt".to_string(),
            thinking: "xhigh".to_string(),
            directory: "/workspace".to_string(),
        };
        let line = chrome.footer_line(80);
        let plain = strip_ansi(&line);
        assert!(plain.starts_with("yolo"));
        assert!(plain.contains("0.0%/272k"));
        assert!(plain.contains("gpt"));
        assert!(plain.contains("xhigh"));
        assert!(plain.contains("/workspace"));
        assert!(!plain.contains("main"));
        assert!(line.contains("\x1b[38;5;81m"));
        assert!(line.contains("\x1b[38;5;177m"));
        assert_eq!(visible_width(&line), 80);
    }

    #[test]
    fn footer_line_never_exceeds_terminal_cols() {
        let chrome = ReplChrome {
            mode: AgentMode::AutoAudit,
            context_ratio: 0.12,
            context_window_tokens: 500_000,
            model: "gpt-5.6-sol".to_string(),
            thinking: "auto".to_string(),
            directory: "/home/snemc/workspace/sai/very/long/path/segment".to_string(),
        };
        for cols in [20usize, 40, 59, 60, 80, 120] {
            let line = chrome.footer_line(cols);
            let width = visible_width(&line);
            assert!(
                width <= cols,
                "cols={cols} width={width} plain={}",
                strip_ansi(&line)
            );
        }
    }

    #[test]
    fn fit_status_segments_avoids_forced_gap_overflow() {
        let left = "yolo  0.0%/500k  gpt-5.6-sol  auto";
        let right = "/home/snemc/workspace/sai";
        let cols = visible_width(left) + visible_width(right);
        let (fitted_left, fitted_right, gap) = fit_status_segments(left, right, cols);
        assert_eq!(gap, 0);
        assert_eq!(
            visible_width(&fitted_left) + gap + visible_width(&fitted_right),
            cols
        );
    }

    fn strip_ansi(text: &str) -> String {
        let mut out = String::new();
        let mut escape = false;
        for ch in text.chars() {
            if ch == '\x1b' {
                escape = true;
                continue;
            }
            if escape {
                if ch == 'm' {
                    escape = false;
                }
                continue;
            }
            out.push(ch);
        }
        out
    }

    #[test]
    fn format_token_k_scales_thousands() {
        assert_eq!(format_token_k(272_000), "272k");
        assert_eq!(format_token_k(1_500), "1.5k");
        assert_eq!(format_token_k(42), "42");
    }

    #[test]
    fn chrome_rule_uses_distinct_color_not_plain_dim() {
        let line = chrome_rule(8);
        assert!(line.contains(CHROME_RULE_STYLE));
        assert!(!line.starts_with("\x1b[2m"));
        assert_eq!(strip_ansi(&line), "────────");
    }
}
