//! 上下文分段条：缓存、新写、压缩余量与保留区共用一条定宽轨道。

use super::sidebar_metrics_view::tokens;
use super::sidebar_widgets::columns;
use crate::i18n::text as t;
use crate::render::panel_palette::{fg, PanelPalette, Rgb};

pub(super) struct ContextBar {
    pub(super) used: usize,
    pub(super) window: usize,
    pub(super) trigger: usize,
    pub(super) cached: Option<usize>,
}

impl ContextBar {
    /// 【终端指标】【压力配色】根据实际压缩点选择常规、接近与超线颜色。
    /// 参数: palette 为当前主题；返回: 新写段与余量读数颜色。
    pub(super) fn tone(&self, palette: PanelPalette) -> Rgb {
        if self.used >= self.trigger {
            palette.removed
        } else if self.used as f64 >= self.trigger as f64 * 0.85 {
            palette.warning
        } else {
            palette.accent
        }
    }

    /// 【终端指标】【容量标题】并列展示当前容量和距压缩余量，超线时显示超出量。
    /// 参数: width 为净列宽；返回: 不超过净宽的 ANSI 标题。
    pub(super) fn heading(&self, width: usize) -> String {
        let palette = PanelPalette::current();
        let left = format!(
            "{}CTX {}{}{}/{}\x1b[39m",
            fg(palette.muted),
            fg(palette.text),
            tokens(self.used),
            fg(palette.subtle),
            tokens(self.window),
        );
        let (label, amount) = if self.used > self.trigger {
            (t("Over +", "超线 +"), self.used - self.trigger)
        } else {
            (t("Compact in ", "距压缩 "), self.trigger - self.used)
        };
        let right = format!(
            "{}{label}{}{}\x1b[39m",
            fg(palette.muted),
            fg(self.tone(palette)),
            tokens(amount),
        );
        columns(&left, &right, width)
    }

    /// 【终端指标】【分段轨道】按物理容量绘制连续色段，仅通过颜色区分缓存、新写、余量和保留区。
    /// 参数: width 为可用列数；返回: 恰好占用 width 列的 ANSI 轨道。
    pub(super) fn track(&self, width: usize) -> String {
        if width == 0 || self.window == 0 {
            return String::new();
        }
        let palette = PanelPalette::current();
        let used = self.used.min(self.window);
        let trigger = self.trigger.min(self.window);
        let reserve_start = (trigger as f64 / self.window as f64 * width as f64).round() as usize;
        // 1. 【终端指标】【非零可见】真实用量至少占一格，缓存与新写均有用量时保留各自色段
        let filled = if used == 0 {
            0
        } else {
            ((used as f64 / self.window as f64 * width as f64).ceil() as usize).min(width)
        };
        let cached = self.cached.map(|cached| cached.min(used));
        let cached_cells = cached.map_or(0, |cached| {
            let count = (cached as f64 / used.max(1) as f64 * filled as f64).round() as usize;
            if filled >= 2 && cached > 0 && cached < used {
                count.clamp(1, filled - 1)
            } else {
                count.min(filled)
            }
        });
        // 2. 【终端指标】【连续色段】全部列用于比例轨道，保留区直接衔接余量段
        let mut line = String::new();
        for col in 0..width {
            let (glyph, tone) = if col < filled {
                let tone = if col >= reserve_start {
                    palette.removed
                } else if col < cached_cells {
                    palette.added
                } else if cached.is_some() {
                    self.tone(palette)
                } else {
                    palette.muted
                };
                ('▰', tone)
            } else if col >= reserve_start {
                ('▱', palette.warning)
            } else {
                ('▱', palette.subtle)
            };
            line.push_str(&fg(tone));
            line.push(glyph);
        }
        line.push_str("\x1b[0m");
        line
    }

    /// 【终端指标】【成分图例】展示当前请求缓存与新写用量，右侧注明保留容量。
    /// 参数: width 为净列宽；返回: 自适应的成分图例，未知缓存使用占位符。
    pub(super) fn legend(&self, width: usize) -> String {
        let palette = PanelPalette::current();
        let cached = self
            .cached
            .map_or_else(|| "--".into(), |value| tokens(value.min(self.used)));
        let fresh = self.cached.map_or_else(
            || "--".into(),
            |value| tokens(self.used.saturating_sub(value)),
        );
        let left = format!(
            "{}■{}{} {}{cached} {}■{}{} {}{fresh}\x1b[39m",
            fg(palette.added),
            fg(palette.muted),
            t("Cache", "缓存"),
            fg(palette.text),
            fg(self.tone(palette)),
            fg(palette.muted),
            t("New", "新写"),
            fg(palette.text),
        );
        let right = format!(
            "{}■{}{} {}\x1b[39m",
            fg(palette.warning),
            fg(palette.subtle),
            t("Reserve", "留"),
            tokens(self.window.saturating_sub(self.trigger)),
        );
        columns(&left, &right, width)
    }
}
