//! 常驻指标独立于可滚动分组，未知读数显示占位符而非伪造零值。

use crate::cli::repl_chrome::ReplChrome;
use crate::cli::repl_runtime::live_usage::LiveTurnUsage;
use crate::render::panel_palette::{fg, PanelPalette};

#[derive(Default)]
pub(super) struct SidebarMetrics {
    rate: Option<f64>,
    context: Option<(usize, usize)>,
    cache: Option<f32>,
}

/// 【终端】【指标空间】参数为侧栏高度，返回指标行数，极矮屏保留主要操作入口
pub(super) fn height(rows: usize) -> usize {
    if rows >= 16 {
        3
    } else {
        0
    }
}

impl SidebarMetrics {
    /// 【终端】【指标快照】参数为输入栏与实时用量，返回本帧只读指标
    pub(super) fn new(chrome: Option<&ReplChrome>, live: &LiveTurnUsage) -> Self {
        Self {
            rate: live.rate(),
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
        }
    }

    /// 【终端】【指标排版】参数为净宽和侧栏高度，返回紧凑等宽的三行指标
    pub(super) fn rows(&self, width: usize, rows: usize) -> Vec<String> {
        let palette = PanelPalette::current();
        let rate = self
            .rate
            .map_or_else(|| "--".into(), |rate| format!("{rate:.1}"));
        let cache = self.cache.map_or_else(
            || "--".into(),
            |cache| format!("{:.0}%", cache.clamp(0.0, 1.0) * 100.0),
        );
        let context = self.context.map_or_else(
            || "--".into(),
            |(used, window)| {
                format!(
                    "{:.1}k/{:.0}k",
                    used as f64 / 1000.0,
                    window as f64 / 1000.0
                )
            },
        );
        [
            format!("~{rate} tok/s  cache {cache}"),
            format!("context {context}"),
            String::new(),
        ]
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
