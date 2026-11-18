//! 常驻上下文分段指标独立于可滚动分组，未知读数显示占位符。

use super::sidebar_context_bar::ContextBar;
use crate::cli::repl_chrome::ReplChrome;
use crate::cli::repl_runtime::live_usage::LiveTurnUsage;
use crate::render::panel_palette::{fg, PanelPalette};

#[derive(Clone, Default)]
pub(super) struct SidebarMetrics {
    rate: Option<f64>,
    context: Option<(usize, usize)>,
    cached: Option<usize>,
    policy: crate::state::CompactionBudgetPolicy,
    cache: Option<f32>,
    output: Option<usize>,
    prompt: Option<usize>,
    directory: String,
    branch: Option<String>,
}

/// 【终端指标】【空间分配】正常高度显示四行分段指标，矮屏使用双行摘要。
/// 参数: rows 为侧栏高度；返回: 指标行数，极矮屏不占用操作入口。
pub(super) fn height(rows: usize) -> usize {
    if rows >= 22 {
        4
    } else if rows >= 16 {
        2
    } else {
        0
    }
}

impl SidebarMetrics {
    /// 【终端指标】【指标快照】收集当前容量、最新请求缓存量和会话压缩策略。
    /// 参数: chrome 为输入栏快照，live 为本轮实时用量；返回: 本帧只读指标。
    pub(super) fn new(chrome: Option<&ReplChrome>, live: &LiveTurnUsage) -> Self {
        let context = chrome.map(|chrome| {
            (
                live.context_prompt_tokens().unwrap_or(
                    (chrome.context_ratio * chrome.context_window_tokens as f32) as usize,
                ),
                chrome.context_window_tokens,
            )
        });
        // 1. 【终端指标】【缓存口径】最新请求有实报时使用其缓存量，否则从会话最后一次请求快照估算
        let cached = live.context_cache_read_tokens().or_else(|| {
            let ratio = chrome?.cache_hit_ratio?;
            let used = context?.0;
            ratio
                .is_finite()
                .then(|| (used as f64 * ratio.clamp(0.0, 1.0) as f64).round() as usize)
        });
        Self {
            rate: live.rate(),
            output: live.output_tokens(),
            prompt: live.context_prompt_tokens(),
            context,
            cached,
            policy: chrome.map_or_else(Default::default, |chrome| chrome.compaction_policy),
            cache: live
                .cache_hit_ratio()
                .or_else(|| chrome.and_then(|chrome| chrome.cache_hit_ratio)),
            directory: chrome.map_or_else(String::new, |chrome| chrome.directory.clone()),
            branch: chrome.and_then(|chrome| chrome.branch.clone()),
        }
    }

    /// 【终端指标】【工作区标题】生成目录与分支双行正文，由纵向布局统一安排位置。
    /// 参数: width 为净宽，rows 为侧栏高度；返回: 顶栏正文行。
    pub(super) fn heading(&self, width: usize, rows: usize) -> Vec<String> {
        super::sidebar_header_path::body_rows(&self.directory, self.branch.as_deref(), width, rows)
    }

    /// 【终端指标】【生成摘要】展示物理占用率、本轮累计缓存命中率与输出速度。
    /// 参数: width 为净列宽；返回: ANSI 摘要行，未知数据使用占位符。
    fn summary(&self, width: usize) -> String {
        let palette = PanelPalette::current();
        let rate = self
            .rate
            .map_or_else(|| "--".into(), |rate| format!("{rate:.1}"));
        let cache = self.cache.map_or_else(
            || "--".into(),
            |value| format!("{:.0}%", value.clamp(0.0, 1.0) * 100.0),
        );
        let output = self
            .output
            .map_or_else(|| "--".into(), super::sidebar_metrics_view::tokens);
        let context = super::sidebar_metrics_view::context(self.context);
        let percent = context
            .split_once(" · ")
            .map_or("--%", |(_, percent)| percent);
        let cache_label = if width >= 42 { "Cache " } else { "C" };
        let speed = if width >= 42 { " tok/s" } else { "/s" };
        let left = format!(
            "{}{percent} · {cache_label}{cache}\x1b[39m",
            fg(palette.muted)
        );
        let right = format!(
            "{}{rate}{}{speed} · out {}{output}\x1b[39m",
            fg(palette.text),
            fg(palette.muted),
            fg(palette.text)
        );
        super::sidebar_widgets::columns(&left, &right, width)
    }

    /// 【终端指标】【分段排版】按屏幕高度选择四行解剖条或双行摘要，宽度不足时统一裁剪。
    /// 参数: width 为净宽，rows 为侧栏高度；返回: 有界 ANSI 指标行。
    pub(super) fn rows(&self, width: usize, rows: usize) -> Vec<String> {
        let context = self
            .context
            .filter(|(_, window)| *window > 0)
            .map(|(used, window)| ContextBar {
                used,
                window,
                trigger: self.policy.trigger_chars(window),
                cached: self.cached,
            });
        let first = context.as_ref().map_or_else(
            || {
                self.prompt.map_or_else(
                    || "CTX -- / -- · --%".into(),
                    |prompt| {
                        format!(
                            "CTX {} / -- · --%",
                            super::sidebar_metrics_view::tokens(prompt)
                        )
                    },
                )
            },
            |bar| bar.heading(width),
        );
        // 1. 【终端指标】【纵向自适应】仅正常高度展示独立轨道与图例，矮屏保留标题和生成摘要
        let lines = if height(rows) == 4 {
            let track = context.as_ref().map_or_else(
                || {
                    format!(
                        "{}{}\x1b[0m",
                        fg(PanelPalette::current().subtle),
                        "▱".repeat(width)
                    )
                },
                |bar| bar.track(width),
            );
            let legend = context.as_ref().map_or_else(
                || "Cache -- · New -- · Reserve --".into(),
                |bar| bar.legend(width),
            );
            vec![first, track, legend, self.summary(width)]
        } else {
            vec![first, self.summary(width)]
        };
        lines
            .into_iter()
            .take(height(rows))
            .map(|line| {
                format!(
                    "{}{}\x1b[0m",
                    fg(PanelPalette::current().muted),
                    crate::render::ansi_clip::clip_ansi_to_width(&line, width, "")
                )
            })
            .collect()
    }
}
