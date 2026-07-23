use crate::render::status_style::{color_running, color_status};
use crate::render::style::TOOL_BULLET;
use crate::render::terminal_text as t;
use crate::render::tool_names::readable_tool_name;
use crate::render::work_status::{format_elapsed, STATUS_PULSE_FRAMES};
use crate::token_counter;
use anyhow::Result;
use crossterm::execute;
use crossterm::terminal::{Clear, ClearType};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::time::Duration;

const TOOL_SUMMARY_DETAIL_LIMIT: usize = 8;

pub(crate) struct StreamSummary {
    reasoning_chars: usize,
    reasoning_lines: usize,
    reasoning_tokens: usize,
    reasoning_source: String,
    /// 思考摘要行是否正在终端上 live 刷新
    reasoning_live: bool,
    /// live 动画帧
    reasoning_frame: usize,
    /// 最近一次 live 耗时
    reasoning_elapsed: Duration,
    tool_stats: BTreeMap<String, ToolStats>,
    readable_tool_names: bool,
}

impl StreamSummary {
    /// 创建流式摘要状态。
    ///
    /// 参数:
    /// - `readable_tool_names`: 是否展示可读工具名称
    ///
    /// 返回:
    /// - 新的流式摘要状态
    pub(crate) fn new(readable_tool_names: bool) -> Self {
        Self {
            reasoning_chars: 0,
            reasoning_lines: 0,
            reasoning_tokens: 0,
            reasoning_source: String::new(),
            reasoning_live: false,
            reasoning_frame: 0,
            reasoning_elapsed: Duration::ZERO,
            tool_stats: BTreeMap::new(),
            readable_tool_names,
        }
    }

    /// 累加推理摘要计数，并立即刷新 live 行。
    ///
    /// 参数:
    /// - `text`: 本次收到的推理文本
    ///
    /// 返回:
    /// - 刷新是否成功
    #[allow(dead_code)]
    pub(crate) fn add_reasoning_text(&mut self, text: &str) -> Result<()> {
        self.add_reasoning_text_with_elapsed(text, self.reasoning_elapsed)
    }

    /// 累加推理摘要计数，并按耗时刷新 live 行。
    ///
    /// 参数:
    /// - `text`: 本次收到的推理文本
    /// - `elapsed`: 本段思考已持续时长
    ///
    /// 返回:
    /// - 刷新是否成功
    pub(crate) fn add_reasoning_text_with_elapsed(
        &mut self,
        text: &str,
        elapsed: Duration,
    ) -> Result<()> {
        // 1. 累计源文与 token
        self.reasoning_source.push_str(text);
        self.reasoning_chars = self.reasoning_source.chars().count();
        self.reasoning_lines =
            self.reasoning_source
                .lines()
                .count()
                .max(if self.reasoning_source.is_empty() {
                    0
                } else {
                    1
                });
        self.reasoning_tokens = token_counter::count(&self.reasoning_source);
        self.reasoning_elapsed = elapsed;
        // 2. 有内容时立刻刷新 live 行（动效 + tokens + 耗时）
        if self.reasoning_chars > 0 {
            self.reasoning_frame = self.reasoning_frame.wrapping_add(1);
            self.render_live_reasoning()?;
        }
        Ok(())
    }

    /// 判断是否存在待固化的推理摘要。
    ///
    /// 返回:
    /// - 是否存在推理摘要
    pub(crate) fn has_reasoning(&self) -> bool {
        self.reasoning_chars > 0
    }

    /// 判断思考摘要 live 行是否仍占用当前终端行。
    ///
    /// 返回:
    /// - 是否 live
    #[cfg(test)]
    pub(crate) fn reasoning_live_active(&self) -> bool {
        self.reasoning_live
    }

    /// 判断是否存在待固化的工具摘要。
    ///
    /// 返回:
    /// - 是否存在工具摘要
    pub(crate) fn has_tools(&self) -> bool {
        !self.tool_stats.is_empty()
    }

    /// 生成推理摘要文本。
    ///
    /// 返回:
    /// - 推理摘要文本
    pub(crate) fn reasoning_text(&self) -> String {
        self.reasoning_text_with_pulse(false)
    }

    /// 生成推理摘要文本。
    ///
    /// 参数:
    /// - `live`: 是否使用跳动前缀
    ///
    /// 返回:
    /// - 推理摘要文本
    fn reasoning_text_with_pulse(&self, live: bool) -> String {
        let tokens = self.reasoning_tokens.max(1);
        let label = if self.reasoning_elapsed.is_zero() {
            t("thinking", "思考").to_string()
        } else {
            format!(
                "{}({})",
                t("thinking", "思考"),
                format_elapsed(self.reasoning_elapsed)
            )
        };
        let prefix = if live {
            STATUS_PULSE_FRAMES[self.reasoning_frame % STATUS_PULSE_FRAMES.len()]
        } else {
            TOOL_BULLET
        };
        format!("{prefix} {} · {} {}", label, tokens, t("tokens", "tokens"))
    }

    /// 用当前计数覆盖终端上的思考摘要 live 行。
    ///
    /// 返回:
    /// - 渲染是否成功
    fn render_live_reasoning(&mut self) -> Result<()> {
        let text = style_summary_text(
            &self.reasoning_text_with_pulse(true),
            SummaryStyle::Reasoning,
        );
        let mut stdout = io::stdout();
        execute!(stdout, Clear(ClearType::CurrentLine))?;
        write!(stdout, "\r{text}")?;
        stdout.flush()?;
        self.reasoning_live = true;
        Ok(())
    }

    /// 固化推理摘要（结束 live 行并换行保留最终计数）。
    ///
    /// 返回:
    /// - 固化是否成功
    /// 静默清空推理摘要状态（不输出，用于 Full 模式已另输出折叠块后）。
    ///
    /// 返回:
    /// - 始终成功
    pub(crate) fn finalize_reasoning_silent(&mut self) -> Result<()> {
        self.reasoning_chars = 0;
        self.reasoning_lines = 0;
        self.reasoning_tokens = 0;
        self.reasoning_source.clear();
        self.reasoning_live = false;
        self.reasoning_frame = 0;
        self.reasoning_elapsed = Duration::ZERO;
        Ok(())
    }

    pub(crate) fn finalize_reasoning(&mut self) -> Result<()> {
        if !self.has_reasoning() {
            return Ok(());
        }
        let text = style_summary_text(&self.reasoning_text(), SummaryStyle::Reasoning);
        let mut stdout = io::stdout();
        if self.reasoning_live {
            // 1. 覆盖 live 行后换行定格
            execute!(stdout, Clear(ClearType::CurrentLine))?;
            write!(stdout, "\r{text}")?;
            writeln!(stdout)?;
        } else {
            writeln!(stdout, "{text}")?;
        }
        stdout.flush()?;
        self.reasoning_chars = 0;
        self.reasoning_lines = 0;
        self.reasoning_tokens = 0;
        self.reasoning_source.clear();
        self.reasoning_live = false;
        self.reasoning_frame = 0;
        self.reasoning_elapsed = Duration::ZERO;
        Ok(())
    }

    /// 记录工具调用开始。
    ///
    /// 参数:
    /// - `name`: 工具名称
    #[cfg(test)]
    pub(crate) fn note_tool_call(&mut self, name: &str) {
        self.tool_stats.entry(name.to_string()).or_default().calls += 1;
    }

    /// 记录工具调用结果。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `ok`: 工具是否成功
    pub(crate) fn note_tool_result(&mut self, name: &str, ok: bool) {
        let stats = self.tool_stats.entry(name.to_string()).or_default();
        if ok {
            stats.ok += 1;
        } else {
            stats.error += 1;
            stats.progress = None;
        }
    }

    /// 更新工具调用进度。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `message`: 进度信息
    pub(crate) fn note_tool_progress(&mut self, name: &str, message: &str) {
        self.tool_stats
            .entry(name.to_string())
            .or_default()
            .progress = Some(message.to_string());
    }

    /// 固化工具调用摘要。
    ///
    /// 返回:
    /// - 固化是否成功
    pub(crate) fn finalize_tools(&mut self) -> Result<()> {
        if self.tool_stats.is_empty() {
            return Ok(());
        }
        let text = self.tool_summary_text();
        println!("{}", style_summary_text(&text, SummaryStyle::Tool));
        self.tool_stats.clear();
        Ok(())
    }

    /// 清除当前实时摘要行（不固化计数，供其它块占用终端行前调用）。
    ///
    /// 返回:
    /// - 清除是否成功
    pub(crate) fn clear_live_lines(&mut self) -> Result<()> {
        if self.reasoning_live {
            let mut stdout = io::stdout();
            execute!(stdout, Clear(ClearType::CurrentLine))?;
            write!(stdout, "\r")?;
            stdout.flush()?;
            self.reasoning_live = false;
        }
        Ok(())
    }

    /// 返回展示用工具名称。
    ///
    /// 参数:
    /// - `name`: 工具原始名称
    ///
    /// 返回:
    /// - 展示名称
    pub(crate) fn display_tool_name<'a>(&self, name: &'a str) -> &'a str {
        if self.readable_tool_names {
            readable_tool_name(name)
        } else {
            name
        }
    }

    /// 生成工具调用摘要文本。
    ///
    /// 返回:
    /// - 工具调用摘要文本
    fn tool_summary_text(&self) -> String {
        let total = tool_totals(&self.tool_stats);
        let mut lines = vec![format!(
            "{TOOL_BULLET} {}: {} {} · {}:{} · {}:{}{}",
            t("tools", "工具"),
            total.calls,
            t("calls", "次"),
            color_status("ok"),
            total.ok,
            color_status("err"),
            total.error,
            running_suffix(total.running)
        )];
        let entries = self
            .tool_stats
            .iter()
            .take(TOOL_SUMMARY_DETAIL_LIMIT)
            .map(|(name, stats)| {
                let header = tool_status_text(self.display_tool_name(name), stats);
                stats.progress.as_ref().map_or(header.clone(), |message| {
                    let progress = message
                        .lines()
                        .filter(|line| !line.trim().is_empty())
                        .map(|line| format!("    {TOOL_BULLET} {}", clip_progress_line(line, 80)))
                        .collect::<Vec<_>>()
                        .join("\n");
                    if progress.is_empty() {
                        header
                    } else {
                        format!("{header}\n{progress}")
                    }
                })
            })
            .map(|entry| format!("  {TOOL_BULLET} {entry}"));
        lines.extend(entries);
        let remaining = self
            .tool_stats
            .len()
            .saturating_sub(TOOL_SUMMARY_DETAIL_LIMIT);
        if remaining > 0 {
            lines.push(format!(
                "  {TOOL_BULLET} ... {} {}",
                remaining,
                t("more tools", "个工具未展开")
            ));
        }
        lines.join("\n")
    }
}

#[derive(Default)]
struct ToolTotals {
    calls: usize,
    ok: usize,
    error: usize,
    running: usize,
}

#[derive(Default)]
pub(crate) struct ToolStats {
    pub(crate) calls: usize,
    pub(crate) ok: usize,
    pub(crate) error: usize,
    pub(crate) progress: Option<String>,
}

/// 汇总工具调用统计。
///
/// 参数:
/// - `tool_stats`: 工具统计表
///
/// 返回:
/// - 汇总后的工具调用统计
fn tool_totals(tool_stats: &BTreeMap<String, ToolStats>) -> ToolTotals {
    tool_stats
        .values()
        .fold(ToolTotals::default(), |mut total, stats| {
            let calls = stats.calls.max(stats.ok + stats.error).max(1);
            total.calls += calls;
            total.ok += stats.ok;
            total.error += stats.error;
            total.running += stats.calls.saturating_sub(stats.ok + stats.error);
            total
        })
}

/// 生成运行中数量后缀。
///
/// 参数:
/// - `running`: 运行中工具数量
///
/// 返回:
/// - 运行中数量文本，空字符串表示没有运行中的工具
fn running_suffix(running: usize) -> String {
    if running == 0 {
        String::new()
    } else {
        format!(" · {}:{running}", color_running(t("running", "运行中")))
    }
}

#[derive(Clone, Copy)]
pub(crate) enum SummaryStyle {
    Reasoning,
    Tool,
}

/// 为摘要文本添加终端样式。
///
/// 参数:
/// - `text`: 摘要文本
/// - `style`: 摘要类型
///
/// 返回:
/// - 带 ANSI 样式的摘要文本
pub(crate) fn style_summary_text(text: &str, style: SummaryStyle) -> String {
    match style {
        SummaryStyle::Reasoning => format!("\x1b[2m\x1b[36m{text}\x1b[0m"),
        SummaryStyle::Tool => format!("\x1b[2m{text}\x1b[0m"),
    }
}

/// 生成工具状态文本。
///
/// 参数:
/// - `name`: 工具展示名称
/// - `stats`: 工具调用统计
///
/// 返回:
/// - 工具状态摘要
pub(crate) fn tool_status_text(name: &str, stats: &ToolStats) -> String {
    let calls = stats.calls.max(stats.ok + stats.error).max(1);
    let running = stats.calls.saturating_sub(stats.ok + stats.error);
    if calls == 1 {
        if running > 0 {
            return format!("{name}×1 {}", color_running(t("running", "运行中")));
        }
        if stats.error > 0 {
            return format!("{name}×1 {}", color_status("err"));
        }
        if stats.ok > 0 {
            return format!("{name}×1 {}", color_status("ok"));
        }
    }
    if running > 0 {
        format!(
            "{name}×{calls} {}:{} {}:{} {}:{}",
            color_running(t("running", "运行中")),
            running,
            color_status("ok"),
            stats.ok,
            color_status("err"),
            stats.error
        )
    } else {
        format!(
            "{name}×{calls} {}:{} {}:{}",
            color_status("ok"),
            stats.ok,
            color_status("err"),
            stats.error
        )
    }
}

/// 压缩进度文本为单行。
///
/// 参数:
/// - `text`: 原始文本
/// - `max_chars`: 最大字符数
///
/// 返回:
/// - 压缩后的文本
fn clip_progress_line(text: &str, max_chars: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= max_chars {
        text
    } else {
        format!(
            "{}...",
            text.chars()
                .take(max_chars.saturating_sub(3))
                .collect::<String>()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_summary_uses_bullet_prefix() {
        let mut summary = StreamSummary::new(false);
        summary.note_tool_call("run_command");

        let output = summary.tool_summary_text();

        assert!(output.starts_with("• "));
        assert!(output.contains("run_command×1"));
    }

    #[test]
    fn reasoning_summary_uses_bullet_prefix() {
        let mut summary = StreamSummary::new(false);
        summary.add_reasoning_text("abc\n").unwrap();

        let output = summary.reasoning_text();

        assert!(output.starts_with("• "));
        assert!(output.contains("思考") || output.contains("thinking"));
        assert!(summary.reasoning_live_active());
        assert!(output.contains("tokens"));
    }

    #[test]
    fn reasoning_summary_token_count_grows_with_chunks() {
        let mut summary = StreamSummary::new(false);
        summary.add_reasoning_text("ab").unwrap();
        let first = summary.reasoning_tokens;
        assert!(first >= 1);
        summary
            .add_reasoning_text("cd\n more tokens here please")
            .unwrap();
        assert!(summary.reasoning_tokens >= first);
        assert!(summary.reasoning_text().contains("tokens"));
    }

    #[test]
    fn tool_progress_lines_use_bullet_prefix() {
        let mut summary = StreamSummary::new(false);
        summary.note_tool_call("edit_file");
        summary.note_tool_progress("edit_file", "replace line\nwrite file");

        let output = summary.tool_summary_text();

        assert!(output.contains("\n    • replace line"));
        assert!(output.contains("\n    • write file"));
        assert!(!output.contains("\n· "));
    }

    #[test]
    fn tool_summary_uses_multiline_compact_layout() {
        let mut summary = StreamSummary::new(false);
        summary.note_tool_call("read_file");
        summary.note_tool_result("read_file", true);
        summary.note_tool_call("web_search");
        summary.note_tool_result("web_search", false);

        let output = summary.tool_summary_text();

        assert!(output.lines().count() >= 3);
        assert!(output
            .lines()
            .next()
            .unwrap()
            .contains("\x1b[32mok\x1b[0m:1"));
        assert!(output
            .lines()
            .next()
            .unwrap()
            .contains("\x1b[31merr\x1b[0m:1"));
        assert!(output.contains("\n  • read_file×1 \x1b[32mok\x1b[0m"));
        assert!(output.contains("\n  • web_search×1 \x1b[31merr\x1b[0m"));
        assert!(!output.contains(", web_search"));
    }

    #[test]
    fn tool_summary_caps_detail_rows() {
        let mut summary = StreamSummary::new(false);
        for index in 0..10 {
            let name = format!("tool_{index}");
            summary.note_tool_call(&name);
            summary.note_tool_result(&name, true);
        }

        let output = summary.tool_summary_text();

        assert!(output.contains("... 2"));
        assert_eq!(
            output
                .lines()
                .filter(|line| line.trim_start().starts_with("• tool_"))
                .count(),
            8
        );
    }
}
