use crate::cli::repl_text::visible_width;
use crate::i18n::text as t;
use crate::render::activity_animation::{render_activity_guide_with_color, render_activity_text};
use crate::render::session_summary::format_k;
use crate::render::status_style::color_status;
use crate::render::transcript::SubagentOverviewEntry;
use crossterm::event::KeyCode;

/// 左栏最少列宽，保证 `1.2k` / `idle` 能右对齐
const TOKEN_COL_MIN: usize = 4;
/// 动效栏与标题栏之间的固定间隔
const COL_GAP: &str = "  ";
/// 运行中引导点色相
const RUNNING_GUIDE: (u8, u8, u8) = (204, 167, 0);
/// 待命引导点色相
const IDLE_GUIDE: (u8, u8, u8) = (97, 175, 239);

/// 底部主/子 agent 切换面板的交互状态。
#[derive(Default)]
pub(super) struct AgentPanelState {
    /// 是否处于选择焦点态（↓ 进入，Esc 退出）
    active: bool,
    /// 当前高亮下标：0 为主 agent，1.. 对应子 agent 条目
    selected: usize,
}

/// 面板按键处理结果。
#[derive(Debug, Eq, PartialEq)]
pub(super) enum AgentPanelAction {
    /// 按键与面板无关，交回常规输入处理
    Ignored,
    /// 按键已被面板消费，仅需重绘底部
    Consumed,
    /// 用户确认选择：`None` 表示主 agent，`Some(cell_index)` 为子 agent
    Apply(Option<usize>),
}

impl AgentPanelState {
    /// 返回面板是否处于焦点态。
    ///
    /// 返回:
    /// - 焦点态标志
    pub(super) fn is_active(&self) -> bool {
        self.active
    }

    /// 尝试进入焦点态（存在子 agent 时才生效）。
    ///
    /// 参数:
    /// - `entries`: 当前子 agent 概览
    ///
    /// 返回:
    /// - 是否进入焦点态
    pub(super) fn activate(&mut self, entries: &[SubagentOverviewEntry]) -> bool {
        if entries.is_empty() {
            return false;
        }
        self.active = true;
        // 默认高亮当前正在查看的子 agent；主视图下高亮主 agent
        self.selected = entries
            .iter()
            .position(|entry| entry.viewing)
            .map(|position| position + 1)
            .unwrap_or(0);
        true
    }

    /// 退出焦点态。
    ///
    /// 返回:
    /// - 无
    pub(super) fn deactivate(&mut self) {
        self.active = false;
    }

    /// 处理焦点态下的按键。
    ///
    /// 参数:
    /// - `code`: 键码
    /// - `entries`: 当前子 agent 概览
    ///
    /// 返回:
    /// - 面板动作
    pub(super) fn handle_key(
        &mut self,
        code: KeyCode,
        entries: &[SubagentOverviewEntry],
    ) -> AgentPanelAction {
        if !self.active {
            return AgentPanelAction::Ignored;
        }
        let total = entries.len() + 1;
        match code {
            KeyCode::Up => {
                self.selected = self.selected.checked_sub(1).unwrap_or(total - 1);
                AgentPanelAction::Consumed
            }
            KeyCode::Down => {
                self.selected = (self.selected + 1) % total;
                AgentPanelAction::Consumed
            }
            KeyCode::Enter => {
                let choice = if self.selected == 0 {
                    None
                } else {
                    entries.get(self.selected - 1).map(|entry| entry.cell_index)
                };
                self.active = false;
                AgentPanelAction::Apply(choice)
            }
            KeyCode::Esc => {
                self.active = false;
                AgentPanelAction::Consumed
            }
            // 其他键：退出面板并交回常规输入处理
            _ => {
                self.active = false;
                AgentPanelAction::Ignored
            }
        }
    }

    /// 渲染面板行（未截断，由沉底面板统一截断到终端宽度）。
    ///
    /// 参数:
    /// - `entries`: 当前子 agent 概览
    /// - `frame`: live 动效帧序号，驱动运行中条目的流光点
    ///
    /// 返回:
    /// - 面板 ANSI 行；无子 agent 时为空
    pub(super) fn panel_lines(
        &self,
        entries: &[SubagentOverviewEntry],
        frame: usize,
    ) -> Vec<String> {
        if entries.is_empty() {
            return Vec::new();
        }
        let running = entries.iter().filter(|entry| entry.running).count();
        let total_tokens = entries.iter().filter_map(|entry| entry.tokens).sum::<u64>();
        let header_left = header_left_plain(total_tokens);
        let col_width = token_column_width(entries, &header_left);
        if !self.active {
            // 非焦点态：左栏合计 tokens（运行中扫光），右栏标题；↓ 进入切换
            let left = render_header_left(total_tokens, running, entries, col_width, frame);
            return vec![format!(
                "    {left}{COL_GAP}\x1b[2m{}: {} ({}) · ↓ {}\x1b[0m",
                t("agents", "智能体"),
                entries.len(),
                format!("{running} {}", t("running", "运行中")),
                t("switch", "切换")
            )];
        }
        let header_left_cell = render_header_left(total_tokens, running, entries, col_width, frame);
        let mut lines = vec![format!(
            "    {header_left_cell}{COL_GAP}\x1b[2m{} · ↑↓ {} · Enter {} · Esc {}\x1b[0m",
            t("agents", "智能体"),
            t("select", "选择"),
            t("view", "查看"),
            t("back", "返回")
        )];
        lines.push(selection_line(
            self.selected == 0,
            &pad_left("", col_width),
            &format!(
                "{} {}",
                t("main agent", "主智能体"),
                t("(overview)", "(总览)")
            ),
        ));
        for (position, entry) in entries.iter().enumerate() {
            let viewing_suffix = if entry.viewing {
                format!(" \x1b[36m({})\x1b[0m", t("viewing", "查看中"))
            } else {
                String::new()
            };
            lines.push(selection_line(
                self.selected == position + 1,
                &render_entry_left(entry, col_width, frame),
                &format!("{}{viewing_suffix}", render_entry_title(entry)),
            ));
        }
        lines
    }
}

impl super::ReplRuntime {
    /// 返回底部 agent 面板是否处于焦点态。
    ///
    /// 返回:
    /// - 焦点态标志
    pub(in crate::cli) fn agent_panel_active(&self) -> bool {
        self.agent_panel.is_active()
    }

    /// 返回当前正在查看的子智能体 ID。
    ///
    /// 供 `/msg` 留言命令把未显式指定目标的消息投递给查看中的子智能体。
    ///
    /// 返回:
    /// - 处于子智能体视图时返回其 ID
    pub(in crate::cli) fn viewing_subagent_id(&self) -> Option<String> {
        self.transcript.viewing_subagent_id().map(str::to_string)
    }

    /// 面板按键核心处理（不负责重绘输入框）。
    ///
    /// 参数:
    /// - `code`: 键码
    ///
    /// 返回:
    /// - 按键被面板消费时返回 true
    fn agent_panel_key_inner(&mut self, code: KeyCode) -> anyhow::Result<bool> {
        let entries = self.transcript.subagent_overview();
        if self.agent_panel.is_active() {
            return match self.agent_panel.handle_key(code, &entries) {
                AgentPanelAction::Consumed => Ok(true),
                AgentPanelAction::Apply(choice) => {
                    // 主 agent：返回主会话视图；子 agent：整体切换到其会话时间线
                    let changed = match choice {
                        None => self.transcript.exit_subagent_view(),
                        Some(cell_index) => self.transcript.enter_subagent_view(cell_index),
                    };
                    if changed {
                        // 视图整体切换：从 source 全量重放
                        self.redraw()?;
                    }
                    Ok(true)
                }
                AgentPanelAction::Ignored => Ok(false),
            };
        }
        if code == KeyCode::Down && self.agent_panel.activate(&entries) {
            self.queue_panel.deactivate();
            return Ok(true);
        }
        Ok(false)
    }

    /// 处理流式阶段的 agent 面板按键（消费后重绘底部输入框）。
    ///
    /// 参数:
    /// - `code`: 键码
    ///
    /// 返回:
    /// - 按键被面板消费时返回 true
    pub(in crate::cli) fn handle_agent_panel_key(&mut self, code: KeyCode) -> anyhow::Result<bool> {
        let was_active = self.agent_panel.is_active();
        let consumed = self.agent_panel_key_inner(code)?;
        // 面板退出（含转交普通处理）也要重绘一次，去掉列表行
        if consumed || was_active {
            self.redraw_stream_composer()?;
        }
        Ok(consumed)
    }

    /// 处理空闲输入阶段的 agent 面板按键（重绘交给输入主循环）。
    ///
    /// 参数:
    /// - `code`: 键码
    ///
    /// 返回:
    /// - 按键被面板消费时返回 true
    pub(in crate::cli) fn handle_agent_panel_idle_key(
        &mut self,
        code: KeyCode,
    ) -> anyhow::Result<bool> {
        self.agent_panel_key_inner(code)
    }
}

/// 渲染单行选择条目：选择箭头与标题着色，左栏动效保持原色。
///
/// 参数:
/// - `selected`: 是否为当前高亮
/// - `left`: 已对齐的 tokens / 状态栏（可含扫光 ANSI）
/// - `title`: 右栏标题
///
/// 返回:
/// - ANSI 行
fn selection_line(selected: bool, left: &str, title: &str) -> String {
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

/// 组装条目右栏：类型 · 描述 · 当前阶段 · 步数 · 时长。
///
/// 面板一行要同时回答「这是谁、在做什么、跑了多久、预算还剩多少」。
/// 按重要性从左往右排，窄终端下先被截掉的是尾部的次要信息。
///
/// 参数:
/// - `entry`: 子智能体概览条目
///
/// 返回:
/// - 右栏纯文本
fn render_entry_title(entry: &SubagentOverviewEntry) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(agent_type) = entry
        .agent_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(clip_chars(agent_type, 14));
    }
    parts.push(clip_chars(entry.label.trim(), 36));
    if let Some(detail) = entry
        .detail
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(clip_chars(detail, 28));
    }
    if let Some((step, max_steps)) = entry.progress {
        parts.push(format!("{step}/{max_steps}"));
    }
    if let Some(elapsed) = entry.elapsed_seconds.filter(|seconds| *seconds > 0) {
        parts.push(format_elapsed(elapsed));
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
fn clip_chars(text: &str, max_chars: usize) -> String {
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
fn format_elapsed(seconds: u64) -> String {
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
fn format_token_plain(tokens: u64) -> String {
    format_k(usize::try_from(tokens).unwrap_or(usize::MAX))
}

/// 标题行左栏纯文本（有合计 tokens 时用数字，否则空，留给引导点）。
fn header_left_plain(total_tokens: u64) -> String {
    if total_tokens > 0 {
        format_token_plain(total_tokens)
    } else {
        String::new()
    }
}

/// 条目左栏纯文本：优先 tokens，否则用状态键占位。
fn entry_left_plain(entry: &SubagentOverviewEntry) -> String {
    match entry.tokens {
        Some(tokens) => format_token_plain(tokens),
        None if entry.running => String::new(),
        None => entry.status.to_string(),
    }
}

/// 计算两栏布局的左栏宽度，保证数字与标题分别对齐。
fn token_column_width(entries: &[SubagentOverviewEntry], header_left: &str) -> usize {
    entries
        .iter()
        .map(|entry| visible_width(&entry_left_plain(entry)))
        .chain(std::iter::once(visible_width(header_left)))
        .max()
        .unwrap_or(0)
        .max(TOKEN_COL_MIN)
}

/// 按可见列宽左补空格，让数字右对齐。
fn pad_left(text: &str, width: usize) -> String {
    let pad = width.saturating_sub(visible_width(text));
    format!("{}{text}", " ".repeat(pad))
}

/// 渲染标题行左栏：有 tokens 则跳动，否则用状态色引导点。
fn render_header_left(
    total_tokens: u64,
    running: usize,
    entries: &[SubagentOverviewEntry],
    width: usize,
    frame: usize,
) -> String {
    if total_tokens > 0 {
        let plain = format_token_plain(total_tokens);
        let pad = " ".repeat(width.saturating_sub(visible_width(&plain)));
        let body = if running > 0 {
            render_activity_text(&plain, frame)
        } else {
            format!("\x1b[2m{plain}\x1b[0m")
        };
        return format!("{pad}{body}");
    }
    let bullet = if running > 0 {
        render_activity_guide_with_color(frame, Some(RUNNING_GUIDE))
    } else if entries.iter().any(|entry| entry.status == "idle") {
        render_activity_guide_with_color(frame, Some(IDLE_GUIDE))
    } else {
        "\x1b[2m•\x1b[0m".to_string()
    };
    format!("{}{bullet}", " ".repeat(width.saturating_sub(1)))
}

/// 渲染条目左栏：运行中的 tokens 走扫光跳动，待命/终态用静态数字或状态色。
fn render_entry_left(entry: &SubagentOverviewEntry, width: usize, frame: usize) -> String {
    if let Some(tokens) = entry.tokens {
        let plain = format_token_plain(tokens);
        let pad = " ".repeat(width.saturating_sub(visible_width(&plain)));
        let body = if entry.running {
            render_activity_text(&plain, frame)
        } else {
            format!("\x1b[2m{plain}\x1b[0m")
        };
        return format!("{pad}{body}");
    }
    if entry.running {
        let bullet = render_activity_guide_with_color(frame, Some(RUNNING_GUIDE));
        return format!("{}{bullet}", " ".repeat(width.saturating_sub(1)));
    }
    let status = color_status(entry.status);
    format!(
        "{}{status}",
        " ".repeat(width.saturating_sub(visible_width(entry.status)))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【agent 面板】条目行同时给出身份、当前动作与进度。
    ///
    /// 此前右栏只有一个「Delegating + 描述」，同屏几个子智能体时
    /// 除了描述本身没有任何可区分信息。
    #[test]
    fn entry_title_combines_identity_and_progress() {
        let entry = SubagentOverviewEntry {
            cell_index: 0,
            label: "诗歌文本多阶段分析".to_string(),
            status: "run",
            running: true,
            viewing: false,
            detail: Some("Reading foo.rs".to_string()),
            tokens: Some(12_300),
            agent_type: Some("explore".to_string()),
            progress: Some((3, 20)),
            elapsed_seconds: Some(134),
        };

        assert_eq!(
            render_entry_title(&entry),
            "explore · 诗歌文本多阶段分析 · Reading foo.rs · 3/20 · 2m14s"
        );
    }

    /// 缺失的可选字段整段省略，不留下空的分隔符。
    #[test]
    fn entry_title_omits_missing_segments() {
        let entry = SubagentOverviewEntry {
            cell_index: 0,
            label: "查资料".to_string(),
            status: "run",
            running: true,
            viewing: false,
            detail: None,
            tokens: None,
            agent_type: None,
            progress: None,
            elapsed_seconds: None,
        };

        assert_eq!(render_entry_title(&entry), "查资料");
    }

    /// 时长在秒 / 分 / 小时之间正确进位。
    #[test]
    fn elapsed_is_formatted_compactly() {
        assert_eq!(format_elapsed(42), "42s");
        assert_eq!(format_elapsed(134), "2m14s");
        assert_eq!(format_elapsed(7_500), "2h05m");
    }

    /// 构造测试概览条目。
    fn entry(cell_index: usize, label: &str, running: bool) -> SubagentOverviewEntry {
        SubagentOverviewEntry {
            cell_index,
            label: label.to_string(),
            status: if running { "run" } else { "ok" },
            running,
            viewing: false,
            detail: None,
            tokens: None,
            agent_type: None,
            progress: None,
            elapsed_seconds: None,
        }
    }

    #[test]
    fn activate_requires_subagents() {
        let mut panel = AgentPanelState::default();
        assert!(!panel.activate(&[]));
        assert!(panel.activate(&[entry(2, "检查项目", true)]));
        assert!(panel.is_active());
    }

    #[test]
    fn keys_cycle_and_apply_selection() {
        let mut panel = AgentPanelState::default();
        let entries = vec![entry(2, "one", true), entry(5, "two", false)];
        panel.activate(&entries);
        assert_eq!(
            panel.handle_key(KeyCode::Down, &entries),
            AgentPanelAction::Consumed
        );
        assert_eq!(
            panel.handle_key(KeyCode::Down, &entries),
            AgentPanelAction::Consumed
        );
        // 主(0) → one(1) → two(2)，Enter 应返回 two 的 cell_index
        assert_eq!(
            panel.handle_key(KeyCode::Enter, &entries),
            AgentPanelAction::Apply(Some(5))
        );
        assert!(!panel.is_active());
    }

    #[test]
    fn enter_on_main_returns_none() {
        let mut panel = AgentPanelState::default();
        let entries = vec![entry(2, "one", true)];
        panel.activate(&entries);
        assert_eq!(
            panel.handle_key(KeyCode::Enter, &entries),
            AgentPanelAction::Apply(None)
        );
    }

    #[test]
    fn escape_and_other_keys_leave_panel() {
        let mut panel = AgentPanelState::default();
        let entries = vec![entry(2, "one", true)];
        panel.activate(&entries);
        assert_eq!(
            panel.handle_key(KeyCode::Esc, &entries),
            AgentPanelAction::Consumed
        );
        assert!(!panel.is_active());
        panel.activate(&entries);
        assert_eq!(
            panel.handle_key(KeyCode::Char('x'), &entries),
            AgentPanelAction::Ignored
        );
        assert!(!panel.is_active());
    }

    #[test]
    fn hint_line_appears_when_inactive() {
        let panel = AgentPanelState::default();
        let entries = vec![entry(2, "one", true)];
        let lines = panel.panel_lines(&entries, 0);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains('↓'));
    }

    #[test]
    fn active_panel_lists_main_and_subagents() {
        let mut panel = AgentPanelState::default();
        let entries = vec![entry(2, "检查项目", true)];
        panel.activate(&entries);
        let lines = panel.panel_lines(&entries, 0);
        assert!(lines.len() >= 3);
        assert!(lines.iter().any(|line| line.contains('❯')));
        assert!(lines.iter().any(|line| line.contains("检查项目")));
    }

    /// 【终端】【agent 面板】运行中走 tokens 跳动，查看中标注清晰。
    #[test]
    fn active_panel_shows_token_cell_and_viewing_marker() {
        let mut panel = AgentPanelState::default();
        let mut running_entry = entry(2, "诗歌分析", true);
        running_entry.tokens = Some(1_200);
        let mut idle_entry = entry(5, "长期重构", false);
        idle_entry.status = "idle";
        idle_entry.viewing = true;
        let entries = vec![running_entry, idle_entry];
        panel.activate(&entries);

        let lines = panel.panel_lines(&entries, 0);
        let joined = lines.join("\n");
        let plain = crate::render::activity_animation::strip_ansi_for_test(&joined);
        assert!(
            plain.contains("1.2k"),
            "运行中条目左栏应展示跳动 tokens: {plain}"
        );
        assert!(
            joined.contains("idle"),
            "待命且无 tokens 的条目左栏应展示 idle: {joined}"
        );
        assert!(joined.contains("查看中") || joined.contains("viewing"));
        assert!(
            !joined.contains("消耗 Token"),
            "长阶段文案不应再挤进标题栏: {joined}"
        );
    }

    /// 【终端】【agent 面板】动效栏与标题栏分列，各行标题起点对齐。
    #[test]
    fn token_and_title_columns_align() {
        let mut panel = AgentPanelState::default();
        let mut running_entry = entry(2, "诗歌分析", true);
        running_entry.tokens = Some(1_200);
        let mut idle_entry = entry(5, "长期重构", false);
        idle_entry.status = "idle";
        idle_entry.tokens = Some(12);
        let entries = vec![running_entry, idle_entry];
        panel.activate(&entries);

        let lines = panel.panel_lines(&entries, 0);
        let titles = [
            t("agents", "智能体"),
            t("main agent", "主智能体"),
            "诗歌分析",
            "长期重构",
        ];
        let starts: Vec<usize> = lines
            .iter()
            .zip(titles)
            .map(|(line, title)| {
                let plain = crate::render::activity_animation::strip_ansi_for_test(line);
                let byte_index = plain
                    .find(title)
                    .unwrap_or_else(|| panic!("missing {title} in {plain}"));
                visible_width(&plain[..byte_index])
            })
            .collect();
        assert_eq!(starts.len(), 4, "标题行与三条条目都应出现: {lines:?}");
        assert!(
            starts.windows(2).all(|pair| pair[0] == pair[1]),
            "标题栏起点必须对齐: {starts:?} / {lines:?}"
        );
        let token_col: Vec<usize> = lines
            .iter()
            .filter_map(|line| {
                let plain = crate::render::activity_animation::strip_ansi_for_test(line);
                ["1.2k", "12"].iter().find_map(|token| {
                    let byte_index = plain.find(token)?;
                    let end = byte_index + token.len();
                    Some(visible_width(&plain[..end]))
                })
            })
            .collect();
        assert_eq!(
            token_col.len(),
            3,
            "标题合计与两条 token 行都应出现: {lines:?}"
        );
        assert!(
            token_col.windows(2).all(|pair| pair[0] == pair[1]),
            "tokens 数字应右对齐在同一栏: {token_col:?} / {lines:?}"
        );
    }
}
