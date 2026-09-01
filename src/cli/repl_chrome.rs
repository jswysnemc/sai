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
    /// 当前轮累计缓存命中率，轮次进行中才有值
    pub(super) cache_hit_ratio: Option<f32>,
    /// 当前会话标题，未命名时可为空
    pub(super) session_title: String,
    /// 底栏左侧附加活动提示，如 `Ctrl+C 停止`
    pub(super) activity: Option<String>,
    /// 底栏左侧常驻的主从角色标记，如 `跟随中`；持有者与单进程时为空
    pub(super) role_badge: Option<String>,
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
            .map(|path| compress_home_prefix(&path.display().to_string()))
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
            cache_hit_ratio: None,
            session_title: String::new(),
            activity: None,
            role_badge: None,
        }
    }

    /// 更新模式（Shift+Tab 切换时）。
    ///
    /// 参数:
    /// - `mode`: 新模式
    pub(super) fn set_mode(&mut self, mode: AgentMode) {
        self.mode = mode;
    }

    /// 写入当前会话标题。
    ///
    /// 参数:
    /// - `title`: 会话标题
    ///
    /// 返回:
    /// - 无
    pub(super) fn set_session_title(&mut self, title: String) {
        self.session_title = title;
    }

    /// 写入底栏活动提示。
    ///
    /// 参数:
    /// - `activity`: 如停止快捷键提示；空则清除
    ///
    /// 返回:
    /// - 无
    pub(super) fn set_activity(&mut self, activity: Option<String>) {
        self.activity = activity;
    }

    /// 写入底栏常驻主从标记。
    ///
    /// 与 `activity` 分开：活动提示只在轮次进行中有值，角色标记只要本终端
    /// 不是会话持有者就一直挂着，让用户随时知道轮次由谁驱动。
    ///
    /// 参数:
    /// - `badge`: 角色标记文本；空则清除
    ///
    /// 返回:
    /// - 无
    pub(super) fn set_role_badge(&mut self, badge: Option<String>) {
        self.role_badge = badge;
    }

    /// 左侧上下文占用文案。
    ///
    /// 轮次进行中带上本轮累计缓存命中率，让长轮次也能看到实时读数。
    ///
    /// 返回:
    /// - 如 `0.0%/272k` 或 `14.1%/1049k cache 96%`
    pub(super) fn context_status(&self) -> String {
        let pct = (self.context_ratio * 100.0).clamp(0.0, 999.9);
        let mut status = format!("{pct:.1}%/{}", format_token_k(self.context_window_tokens));
        if let Some(ratio) = self.cache_hit_ratio {
            status.push_str(&format!(" cache {:.0}%", (ratio * 100.0).clamp(0.0, 100.0)));
        }
        status
    }

    /// 按轮次进行中的实时读数覆盖上下文与缓存显示。
    ///
    /// 参数:
    /// - `prompt_tokens`: 最近一次请求 provider 实报的上下文占用
    /// - `cache_hit_ratio`: 本轮累计缓存命中率
    ///
    /// 返回:
    /// - 无
    pub(super) fn apply_live_usage(
        &mut self,
        prompt_tokens: Option<usize>,
        cache_hit_ratio: Option<f32>,
    ) {
        if let Some(tokens) = prompt_tokens {
            if self.context_window_tokens > 0 {
                self.context_ratio = tokens as f32 / self.context_window_tokens as f32;
            }
        }
        if cache_hit_ratio.is_some() {
            self.cache_hit_ratio = cache_hit_ratio;
        }
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

    /// 底栏整行：左侧模式/上下文/模型/思考，右侧目录。
    ///
    /// 参数:
    /// - `cols`: 终端列数（面板内为扣除彩条后的净宽）
    ///
    /// 返回:
    /// - 已着色状态行
    pub(super) fn footer_line(&self, cols: usize) -> String {
        self.footer_line_with_activity(cols, self.activity.as_deref())
    }

    /// 底栏整行，左侧可附加当前工作状态。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    /// - `activity`: 如 `Working 12s`
    ///
    /// 返回:
    /// - 已着色状态行
    pub(super) fn footer_line_with_activity(&self, cols: usize, activity: Option<&str>) -> String {
        let badge = self
            .role_badge
            .as_deref()
            .filter(|badge| !badge.is_empty())
            .map(|badge| format!("{badge}  "))
            .unwrap_or_default();
        let left_plain = match activity.filter(|text| !text.is_empty()) {
            Some(activity) => format!(
                "{badge}{activity}  {}  {}  {}  {}",
                self.mode_plain(),
                self.context_status(),
                self.model,
                self.thinking
            ),
            None => format!(
                "{badge}{}  {}  {}  {}",
                self.mode_plain(),
                self.context_status(),
                self.model,
                self.thinking
            ),
        };
        self.compose_footer_line(cols, &left_plain)
    }

    /// 按净宽裁剪并着色底栏左右两段。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    /// - `left_plain`: 左侧纯文本
    ///
    /// 返回:
    /// - 已着色状态行
    fn compose_footer_line(&self, cols: usize, left_plain: &str) -> String {
        let cols = cols.max(1);
        let pad = CHROME_FOOTER_SIDE_PAD.min(cols.saturating_sub(1) / 2);
        let inner = cols.saturating_sub(pad.saturating_mul(2)).max(1);
        let right_plain = footer_right_text(&self.session_title, &self.directory);
        // 1. 在扣除左右外边距后的净宽上裁剪，避免贴边
        let (left_text, right_text, gap) = fit_status_segments(&left_plain, &right_plain, inner);
        // 2. 裁剪后再着色，避免 ANSI 干扰宽度计算
        let left = colorize_left_status(self.mode, &left_text, self.context_ratio);
        let right = if right_text.is_empty() {
            String::new()
        } else {
            color_directory(&right_text)
        };
        format!(
            "{}{left}{}{right}{}",
            " ".repeat(pad),
            " ".repeat(gap),
            " ".repeat(pad)
        )
    }
}

/// 底栏右侧：会话标题（截断）加工作目录。
///
/// 参数:
/// - `title`: 会话标题
/// - `directory`: 压缩后的工作目录
///
/// 返回:
/// - 右侧纯文本
fn footer_right_text(title: &str, directory: &str) -> String {
    let title = title.trim();
    if title.is_empty() {
        return directory.to_string();
    }
    let short: String = title.chars().take(16).collect();
    if title.chars().count() > 16 {
        format!("{short}…  {directory}")
    } else {
        format!("{short}  {directory}")
    }
}

/// 把家目录前缀压缩成 `~`。
///
/// 底栏右侧只有一段空间，深层路径会把左侧的模式与模型挤掉。
/// Windows 的家目录同样来自 BaseDirs，因此三个平台共用一套逻辑。
///
/// 参数:
/// - `path`: 当前工作目录的显示文本
///
/// 返回:
/// - 压缩后的路径；不在家目录内时原样返回
pub(super) fn compress_home_prefix(path: &str) -> String {
    let Some(home) = directories::BaseDirs::new()
        .map(|dirs| dirs.home_dir().display().to_string())
        .filter(|home| !home.is_empty())
    else {
        return path.to_string();
    };
    compress_with_home(path, &home)
}

/// 按给定家目录压缩路径前缀。
///
/// 参数:
/// - `path`: 当前工作目录
/// - `home`: 家目录
///
/// 返回:
/// - 压缩后的路径
fn compress_with_home(path: &str, home: &str) -> String {
    if path == home {
        return "~".to_string();
    }
    let separator = std::path::MAIN_SEPARATOR;
    // 只认目录边界，避免 /home/snemc-backup 被误压成 ~-backup
    let prefix = format!("{home}{separator}");
    match path.strip_prefix(&prefix) {
        Some(rest) => format!("~{separator}{rest}"),
        None => path.to_string(),
    }
}

/// 给模型名称使用稳定的重点颜色。
fn color_model(value: &str) -> String {
    format!("\x1b[38;5;110m{value}\x1b[0m")
}

/// 给思考等级使用独立颜色。
fn color_thinking(value: &str) -> String {
    format!("\x1b[38;5;109m{value}\x1b[0m")
}

/// 给右侧当前目录使用弱化但可辨识的颜色。
fn color_directory(value: &str) -> String {
    format!("\x1b[38;5;103m{value}\x1b[0m")
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
    let left_w = visible_width(&left);
    let right_w = visible_width(&right);

    // 1. 左右总宽不超过终端：gap 填剩余列，贴满时为 0（不再额外预留分隔列）
    if left_w + right_w <= cols {
        return (left, right, cols - left_w - right_w);
    }

    // 2. 放不下时右侧先让位，优先保留模式/上下文/模型
    if left_w < cols {
        right = truncate_to_width(&right, cols - left_w);
        let used = left_w + visible_width(&right);
        return (left, right, cols.saturating_sub(used));
    }

    // 3. 左侧单独已超宽：只保留左侧并裁剪
    left = truncate_to_width(&left, cols);
    (left, String::new(), 0)
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
    let thinking = parts
        .get(3..)
        .map(|rest| rest.join("  "))
        .unwrap_or_default();

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

/// 输入面板背景（暗色终端上的深色抬升条）。
const CHROME_PANEL_BG: &str = "\x1b[48;5;235m";
/// 输入行提示符形态。
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum ChromeInputPrefix {
    /// 首行普通消息：弱化灰 `→`
    Message,
    /// 首行以 `!` 开头：橙色 `$`，表示将执行本地命令
    Shell,
    /// 折行续行：仅保留缩进，不重复提示符
    Continuation,
}

impl ChromeInputPrefix {
    /// 返回带样式的前缀文本（宽度恒为 `CHROME_INPUT_PREFIX_COLS`）。
    ///
    /// 用 `\x1b[39m` 只重置前景，保持整行背景连续。
    fn render(self) -> &'static str {
        match self {
            Self::Message => " \x1b[38;5;245m→\x1b[39m ",
            Self::Shell => " \x1b[38;5;208m$\x1b[39m ",
            Self::Continuation => "   ",
        }
    }
}
/// 输入提示符占用的列数（含左内边距与间隔）。
pub(super) const CHROME_INPUT_PREFIX_COLS: usize = 3;
/// 输入正文上下各留的空白行数。
pub(super) const CHROME_INPUT_PAD_ROWS: u16 = 1;
/// 输入条内部上下各留的背景内边距行数（增加输入框视觉厚度）。
pub(super) const CHROME_INPUT_INNER_PAD_ROWS: u16 = 1;
/// 底栏状态左右外边距：与输入条的文字起点对齐。
pub(super) const CHROME_FOOTER_SIDE_PAD: usize = CHROME_INPUT_PREFIX_COLS;

/// 输入行：深色背景通栏 + 按行形态渲染提示符。
pub(super) fn chrome_input_row(prefix: ChromeInputPrefix, content: &str, cols: usize) -> String {
    let inner = chrome_input_content_cols(cols);
    let width = visible_width(content);
    let glyph = prefix.render();
    // 正文样式中的 reset 会打断整行背景（占位提示自带 \x1b[0m），
    // reset 后立即恢复面板底色，保证背景条贯穿整行
    let content = content.replace("\x1b[0m", &format!("\x1b[0m{CHROME_PANEL_BG}"));
    if width >= inner {
        format!(
            "{CHROME_PANEL_BG}{glyph}{}\x1b[0m",
            truncate_ansi_to_width(&content, inner)
        )
    } else {
        format!(
            "{CHROME_PANEL_BG}{glyph}{content}{}\x1b[0m",
            " ".repeat(inner - width)
        )
    }
}

/// 输入区可用列数（扣除提示符前缀）。
pub(super) fn chrome_input_content_cols(cols: usize) -> usize {
    cols.saturating_sub(CHROME_INPUT_PREFIX_COLS).max(1)
}

/// 输入条内部的背景内边距行（整行底色，无内容）。
pub(super) fn chrome_input_pad_row(cols: usize) -> String {
    format!("{CHROME_PANEL_BG}{}\x1b[0m", " ".repeat(cols.max(1)))
}

/// 截断含 ANSI 的文本到指定显示宽度。
fn truncate_ansi_to_width(text: &str, width: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    if width == 0 {
        return String::new();
    }
    if visible_width(text) <= width {
        return text.to_string();
    }
    let mut out = String::new();
    let mut used = 0usize;
    let mut index = 0usize;
    while index < text.len() {
        let ch = text[index..].chars().next().unwrap_or_default();
        if ch == '\x1b' {
            let end = crate::render::terminal_image::escape_sequence_end(text, index);
            out.push_str(&text[index..end]);
            index = end.max(index + ch.len_utf8());
            continue;
        }
        let char_width = ch.width().unwrap_or(0);
        if used.saturating_add(char_width) > width {
            break;
        }
        out.push(ch);
        used = used.saturating_add(char_width);
        index += ch.len_utf8();
    }
    // 截断后恢复面板底色，避免后续 padding 丢背景
    out.push_str("\x1b[0m");
    out.push_str(CHROME_PANEL_BG);
    out
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

/// opencode 风格 chrome 固定占用行数：输入上下各一行空白 + 底部状态行。
///
/// 返回:
/// - 固定行数
pub(super) fn chrome_fixed_rows() -> u16 {
    CHROME_INPUT_PAD_ROWS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造测试用底栏 chrome。
    ///
    /// 返回:
    /// - chrome 状态
    fn test_chrome() -> ReplChrome {
        ReplChrome {
            mode: AgentMode::Yolo,
            context_ratio: 0.0,
            context_window_tokens: 200_000,
            model: "gpt".to_string(),
            thinking: "auto".to_string(),
            directory: "/workspace".to_string(),
            cache_hit_ratio: None,
            session_title: String::new(),
            activity: None,
            role_badge: None,
        }
    }

    /// 角色标记常驻底栏，且排在活动提示之前。
    #[test]
    fn role_badge_sits_at_the_left_of_the_footer() {
        let mut chrome = test_chrome();
        assert!(!chrome.footer_line(60).contains("跟随中"));

        chrome.set_role_badge(Some("跟随中".to_string()));
        assert!(chrome.footer_line(60).contains("跟随中"));
        // 活动提示（Ctrl+C 停止）出现时角色标记仍在
        let busy = chrome.footer_line_with_activity(60, Some("Ctrl+C"));
        assert!(busy.contains("跟随中"));
        assert!(busy.contains("Ctrl+C"));
        assert!(busy.find("跟随中").unwrap() < busy.find("Ctrl+C").unwrap());
    }

    /// 【TUI】【实时用量】验证实报读数覆盖上下文占比并带出缓存命中。
    #[test]
    fn live_usage_overrides_context_status() {
        let mut chrome = test_chrome();
        assert_eq!(chrome.context_status(), "0.0%/200k");

        chrome.apply_live_usage(Some(50_000), Some(0.96));

        assert_eq!(chrome.context_status(), "25.0%/200k cache 96%");
    }

    /// 【TUI】【实时用量】验证轮次尚无实报读数时保留原快照显示。
    #[test]
    fn live_usage_without_reading_keeps_snapshot() {
        let mut chrome = test_chrome();
        chrome.context_ratio = 0.141;

        chrome.apply_live_usage(None, None);

        assert_eq!(chrome.context_status(), "14.1%/200k");
    }

    /// 【TUI】【底栏路径】验证家目录压缩为 ~ 且只认目录边界。
    #[test]
    fn home_prefix_is_compressed_on_directory_boundary() {
        let sep = std::path::MAIN_SEPARATOR;
        let home = format!("{sep}home{sep}snemc");

        assert_eq!(compress_with_home(&home, &home), "~");
        assert_eq!(
            compress_with_home(&format!("{home}{sep}workspace{sep}sai"), &home),
            format!("~{sep}workspace{sep}sai")
        );
        // 同名前缀的兄弟目录不能被压缩
        assert_eq!(
            compress_with_home(&format!("{home}-backup"), &home),
            format!("{home}-backup")
        );
        assert_eq!(
            compress_with_home(&format!("{sep}etc{sep}sai"), &home),
            format!("{sep}etc{sep}sai")
        );
    }

    #[test]
    fn status_line_keeps_left_and_right() {
        let line = chrome_status_line("0.0%/272k (auto)", "gpt · xhigh", 40);
        assert!(line.contains("0.0%/272k (auto)"));
        assert!(line.contains("gpt · xhigh"));
    }

    #[test]
    fn footer_puts_session_title_before_directory() {
        let mut chrome = test_chrome();
        chrome.set_session_title("demo-session".to_string());
        let line = chrome.footer_line(80);
        let plain = strip_ansi(&line);
        let title = plain.find("demo-session").expect("title");
        let directory = plain.find("/workspace").expect("directory");
        assert!(title < directory, "{plain}");
    }

    #[test]
    fn footer_puts_activity_before_mode() {
        let chrome = test_chrome();
        let line = chrome.footer_line_with_activity(80, Some("Working 12s"));
        let plain = crate::render::activity_animation::strip_ansi_for_test(&line);
        let work = plain.find("Working 12s").expect("activity");
        let mode = plain.find("yolo").expect("mode");
        assert!(work < mode, "{plain}");
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
            cache_hit_ratio: None,
            session_title: String::new(),
            activity: None,
            role_badge: None,
        };
        let line = chrome.footer_line(80);
        let plain = strip_ansi(&line);
        assert!(
            plain.starts_with(' '),
            "footer must keep left outer pad: {plain:?}"
        );
        assert!(
            plain.ends_with(' '),
            "footer must keep right outer pad: {plain:?}"
        );
        assert!(plain.contains("yolo"));
        assert!(plain.contains("0.0%/272k"));
        assert!(plain.contains("gpt"));
        assert!(plain.contains("xhigh"));
        assert!(plain.contains("/workspace"));
        assert!(!plain.contains("main"));
        assert!(line.contains("\x1b[38;5;110m"));
        assert!(line.contains("\x1b[38;5;109m"));
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
            cache_hit_ratio: None,
            session_title: String::new(),
            activity: None,
            role_badge: None,
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
    fn input_row_prefix_follows_line_role() {
        let first = chrome_input_row(ChromeInputPrefix::Message, "hello", 20);
        assert!(first.contains(CHROME_PANEL_BG));
        assert!(first.contains('→'));
        assert!(first.contains("hello"));
        // shell 模式换 $ 提示符
        let shell = chrome_input_row(ChromeInputPrefix::Shell, "!ls", 20);
        assert!(shell.contains('$'));
        assert!(!shell.contains('→'));
        // 续行只保留缩进
        let cont = chrome_input_row(ChromeInputPrefix::Continuation, "wrapped", 20);
        assert!(!cont.contains('→') && !cont.contains('$'));
        assert!(cont.contains("wrapped"));
        // 输入上方一行空白
        assert_eq!(chrome_fixed_rows(), 1);
    }
}
