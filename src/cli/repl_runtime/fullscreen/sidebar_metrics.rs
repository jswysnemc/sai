//! 常驻指标独立于可滚动分组，未知读数显示占位符而非伪造零值。

use crate::cli::repl_chrome::ReplChrome;
use crate::cli::repl_runtime::live_usage::LiveTurnUsage;
use crate::render::panel_palette::{fg, PanelPalette};

#[derive(Clone, Default)]
pub(super) struct SidebarMetrics {
    rate: Option<f64>,
    context: Option<(usize, usize)>,
    cache: Option<f32>,
    output: Option<usize>,
    prompt: Option<usize>,
    directory: String,
    branch: Option<String>,
}

/// 【终端】【指标空间】参数为侧栏高度，返回指标行数，极矮屏保留主要操作入口
pub(super) fn height(rows: usize) -> usize {
    if rows >= 16 {
        2
    } else {
        0
    }
}

impl SidebarMetrics {
    /// 【终端】【指标快照】参数为输入栏与实时用量，返回本帧只读指标
    pub(super) fn new(chrome: Option<&ReplChrome>, live: &LiveTurnUsage) -> Self {
        Self {
            rate: live.rate(),
            output: live.output_tokens(),
            prompt: live.context_prompt_tokens(),
            context: chrome.map(|chrome| {
                (
                    live.context_prompt_tokens().unwrap_or(
                        (chrome.context_ratio * chrome.context_window_tokens as f32) as usize,
                    ),
                    chrome.context_window_tokens,
                )
            }),
            cache: live
                .cache_hit_ratio()
                .or_else(|| chrome.and_then(|chrome| chrome.cache_hit_ratio)),
            directory: chrome.map_or_else(String::new, |chrome| chrome.directory.clone()),
            branch: chrome.and_then(|chrome| chrome.branch.clone()),
        }
    }

    /// 【终端指标】【工作区标题】参数：width 为净宽，rows 为高度；返回：品牌、工作区与分支的单行标题
    pub(super) fn heading(&self, width: usize, rows: usize) -> Vec<String> {
        // 【终端指标】【控件避让】为终端右上角开关保留四列，避免覆盖分支文字
        let width = width.saturating_sub(4);
        let title = super::sidebar_widgets::safe_text(&self.directory);
        let branch = self.branch.as_deref().unwrap_or_default();
        let mut header = super::pane_header::render(&title, width, rows);
        if let Some(line) = header.first_mut() {
            *line = super::sidebar_widgets::columns(
                line.trim_end(),
                &super::sidebar_widgets::safe_text(branch),
                width,
            );
        }
        header
    }

    /// 【终端】【指标排版】参数为净宽和侧栏高度，返回上下文数值、分段条和缓存生成摘要
    pub(super) fn rows(&self, width: usize, rows: usize) -> Vec<String> {
        let palette = PanelPalette::current();
        let rate = self
            .rate
            .map_or_else(|| "--".into(), |rate| format!("{rate:.1}"));
        let cache = self.cache.map_or_else(
            || "--".into(),
            |cache| format!("{:.0}%", cache.clamp(0.0, 1.0) * 100.0),
        );
        let context = if self.context.is_none() {
            self.prompt.map_or_else(
                || super::sidebar_metrics_view::context(None),
                |prompt| format!("{} / -- · --%", super::sidebar_metrics_view::tokens(prompt)),
            )
        } else {
            super::sidebar_metrics_view::context(self.context)
        };
        let output = self
            .output
            .map_or_else(|| "--".into(), super::sidebar_metrics_view::tokens);
        // 1. 【终端指标】【紧凑读数】数值与自适应比例条共用一行，填充中段空隙并区分标签与数值对比度
        let (numbers, percent) = context.split_once(" · ").unwrap_or((&context, ""));
        let spare = width.saturating_sub(
            crate::cli::repl_text::visible_width(numbers)
                + crate::cli::repl_text::visible_width(percent)
                + 3,
        );
        let gauge = super::sidebar_metrics_view::bar(self.context, spare.min(24));
        let styled_numbers = if let Some((used, window)) = numbers.split_once(" / ") {
            format!(
                "{}{used}{} / {}{window}\x1b[39m",
                fg(palette.text),
                fg(palette.muted),
                fg(palette.muted)
            )
        } else {
            format!("{}{numbers}\x1b[39m", fg(palette.text))
        };
        let ratio = self
            .context
            .filter(|(_, window)| *window > 0)
            .map(|(used, window)| used as f64 / window as f64);
        let percent_color = if ratio.is_some_and(|ratio| ratio >= 0.9) {
            palette.removed
        } else if ratio.is_some_and(|ratio| ratio >= 0.72) {
            palette.warning
        } else {
            palette.text
        };
        let styled_percent = format!("{}\x1b[1m{percent}\x1b[22m\x1b[39m", fg(percent_color));
        let left_part = if spare == 0 {
            styled_numbers
        } else {
            format!("{styled_numbers} {gauge}")
        };
        let first = super::sidebar_widgets::columns(&left_part, &styled_percent, width);
        let sep = format!(" {}· ", fg(palette.subtle));
        let second = format!(
            "{}Cache {}{cache}{sep}{}{rate} {}tok/s{sep}{}out {}{output}\x1b[0m",
            fg(palette.muted),
            fg(palette.text),
            fg(palette.text),
            fg(palette.muted),
            fg(palette.muted),
            fg(palette.text)
        );
        [first, second]
            .into_iter()
            .take(height(rows))
            .map(|line| {
                format!(
                    "{}{}\x1b[0m",
                    fg(palette.muted),
                    crate::render::ansi_clip::clip_ansi_to_width(&line, width, "")
                )
            })
            .collect()
    }
}
