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

    /// 【终端指标】【工作区标题】生成工作区目录与 Git 分支双行正文，顶部下沉偏移由纵向布局统一排布。
    /// 参数: width 为净宽，rows 为侧栏高度；返回: 顶栏正文行。
    pub(super) fn heading(&self, width: usize, rows: usize) -> Vec<String> {
        super::sidebar_header_path::body_rows(
            &self.directory,
            self.branch.as_deref(),
            width,
            rows,
        )
    }

    /// 【终端】【指标排版】参数为净宽和侧栏高度，返回上下文数值、线性轨道和按有效负载着色的生成摘要
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
        // 1. 【终端指标】【负载权重】零负载状态弱化百分比与零值字重，仅在高水位预警时加粗强调
        let (numbers, percent) = context.split_once(" · ").unwrap_or((&context, ""));
        let spare = width.saturating_sub(
            crate::cli::repl_text::visible_width(numbers)
                + crate::cli::repl_text::visible_width(percent)
                + 3,
        );
        let gauge = super::sidebar_metrics_view::bar(self.context, spare.min(22));
        let ratio = self
            .context
            .filter(|(_, window)| *window > 0)
            .map(|(used, window)| used as f64 / window as f64);
        let has_load = ratio.is_some_and(|value| value > 0.0)
            || self.prompt.is_some_and(|prompt| prompt > 0);
        let styled_numbers = if let Some((used, window)) = numbers.split_once(" / ") {
            let (used_tone, window_tone) = if has_load {
                (palette.text, palette.muted)
            } else {
                (palette.muted, palette.subtle)
            };
            format!(
                "{}{used}{} / {}{window}\x1b[39m",
                fg(used_tone),
                fg(window_tone),
                fg(window_tone)
            )
        } else {
            let tone = if has_load { palette.text } else { palette.muted };
            format!("{}{numbers}\x1b[39m", fg(tone))
        };
        let styled_percent = if ratio.is_some_and(|value| value >= 0.9) {
            format!("{}\x1b[1m{percent}\x1b[22m\x1b[39m", fg(palette.removed))
        } else if ratio.is_some_and(|value| value >= 0.72) {
            format!("{}\x1b[1m{percent}\x1b[22m\x1b[39m", fg(palette.warning))
        } else if has_load {
            format!("{}{percent}\x1b[39m", fg(palette.text))
        } else {
            format!("{}{percent}\x1b[39m", fg(palette.muted))
        };
        let left_part = if spare == 0 {
            styled_numbers
        } else {
            format!("{styled_numbers} {gauge}")
        };
        let first = super::sidebar_widgets::columns(&left_part, &styled_percent, width);
        // 2. 【终端指标】【空闲降噪】未产生数据的占位读数整体退至静默色，有实测读数时点亮标签与数值
        let sep = format!(" {}· ", fg(palette.subtle));
        let cache_part = if self.cache.is_some() {
            format!("{}Cache {}{cache}", fg(palette.muted), fg(palette.text))
        } else {
            format!("{}Cache {cache}", fg(palette.subtle))
        };
        let rate_part = if self.rate.is_some() {
            format!("{}{rate} {}tok/s", fg(palette.text), fg(palette.muted))
        } else {
            format!("{}{rate} tok/s", fg(palette.subtle))
        };
        let out_part = if self.output.is_some() {
            format!("{}out {}{output}", fg(palette.muted), fg(palette.text))
        } else {
            format!("{}out {output}", fg(palette.subtle))
        };
        let second = format!("{cache_part}{sep}{rate_part}{sep}{out_part}\x1b[0m");
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
