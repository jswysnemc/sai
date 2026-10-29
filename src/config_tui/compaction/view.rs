//! 压缩策略卡片：模式选择、实时阈值和快捷键说明。

use super::card::{self, ACCENT, BORDER, BRAND, MUTED, RESET, SELECTED};
use super::draft::{format_percent, Draft, RESERVE_PRESETS};
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    queue,
    terminal::{self, Clear, ClearType},
};
use std::io;

/// 【上下文】【策略预览】当前模型窗口与配置作用范围。
pub(super) struct PreviewContext {
    pub window: usize,
    pub used: Option<usize>,
    pub scope: String,
}

/// 【上下文】【模式选择】显示两种互斥策略，预留模式展开快捷档位。
/// 参数：`draft` 为草稿；返回：两个可独立换行的模式控件。
fn selectors(draft: &Draft) -> [String; 2] {
    let ratio = if draft.selected == 0 && draft.input.is_some() {
        draft.input_display()
    } else {
        let value = format!("{}%", format_percent(draft.policy.ratio));
        if draft.selected == 0 {
            format!("{SELECTED} ← {value} → {RESET}")
        } else {
            value
        }
    };
    let reserve = if draft.selected == 1 && draft.input.is_some() {
        draft.input_display()
    } else if draft.selected == 1 {
        let mut values = RESERVE_PRESETS.to_vec();
        if !values.contains(&draft.policy.reserve_tokens) {
            values.push(draft.policy.reserve_tokens);
            values.sort_unstable();
        }
        values
            .into_iter()
            .map(|value| {
                let text = if value == 0 {
                    t("off", "关闭").into()
                } else {
                    card::compact(value)
                };
                if value == draft.policy.reserve_tokens {
                    format!("{SELECTED}{ACCENT} ← {text} → {RESET}")
                } else {
                    format!("{MUTED}{text}{RESET}")
                }
            })
            .collect::<Vec<_>>()
            .join(&format!("{BORDER} | {RESET}"))
    } else {
        card::compact(draft.policy.reserve_tokens)
    };
    [
        (t("Usage ratio", "按占用比例"), ratio),
        (t("Safety reserve", "按安全预留"), reserve),
    ]
    .map(|(label, value)| format!("{label} [{value}{RESET}]"))
    .into_iter()
    .enumerate()
    .map(|(index, text)| {
        let (style, radio) = if index == draft.selected {
            (ACCENT, "●")
        } else {
            (MUTED, "○")
        };
        format!("{style}{radio} {text}{RESET}")
    })
    .collect::<Vec<_>>()
    .try_into()
    .unwrap()
}

/// 【上下文】【卡片排版】宽屏保持五行，窄屏拆分控件和统计，不强制最小宽度。
/// 参数：草稿、上下文与终端列数；返回：完整 ANSI 卡片。
pub(super) fn frame(draft: &Draft, context: &PreviewContext, cols: usize) -> Vec<String> {
    let width = cols.clamp(1, 88);
    let inner = width.saturating_sub(4).max(1);
    let preview = draft.preview();
    let policy = preview.as_ref().copied().unwrap_or(draft.policy);
    let choices = selectors(draft);
    let mut body = Vec::new();
    let combined = choices.join("   ");
    if crate::render::ansi_clip::ansi_visible_width(&combined) <= inner {
        body.push(combined);
    } else {
        body.extend(choices);
    }
    if context.window == 0 {
        body.push(
            t(
                "Window unknown; preview unavailable",
                "窗口未知，暂时无法预览",
            )
            .into(),
        );
    } else {
        body.push(super::gauge::bar(context, policy, inner));
        body.push(super::gauge::summary(context, policy));
    }
    if draft.reset {
        body.push(
            t(
                "Defaults restored in draft; save to apply",
                "默认值已恢复到草稿，保存后生效",
            )
            .into(),
        );
    }
    if let Some(error) = draft
        .error
        .clone()
        .or_else(|| preview.err().map(|error| error.to_string()))
    {
        body.push(format!("\x1b[31m{error}{RESET}"));
    }
    let help = if draft.input.is_some() {
        t(
            "Enter apply · Esc undo · Ctrl+A select · Ctrl+U clear",
            "Enter 确认 · Esc 撤销 · Ctrl+A 全选 · Ctrl+U 清空",
        )
    } else {
        t(
            "Tab mode · ←→ adjust · 0–3 presets · e edit · r reset · Enter save · Esc cancel",
            "Tab 切换 · ←→ 调节 · 0–3 档位 · e 输入 · r 恢复 · Enter 保存 · Esc 取消",
        )
    };
    let title = if width < 60 {
        format!(
            "{BRAND}{}{RESET} {MUTED}[{}]{RESET}",
            t("AUTO-COMPACT", "自动压缩策略"),
            card::compact(context.window)
        )
    } else {
        format!(
            "{BRAND}{}{RESET} · {MUTED}{} [{}]{RESET}",
            t("AUTO-COMPACT", "自动压缩策略"),
            context.scope,
            card::compact(context.window)
        )
    };
    card::frame(&title, &body, help, width)
}

/// 【上下文】【全局绘制】在配置界面自身画布中居中显示同一张策略卡片。
/// 参数：输出、草稿与上下文；返回：绘制结果。
pub(super) fn draw(stdout: &mut io::Stdout, draft: &Draft, context: &PreviewContext) -> Result<()> {
    let (cols, rows) = terminal::size()?;
    let lines = card::fit_height(frame(draft, context, usize::from(cols)), usize::from(rows));
    crate::config_tui::ui::begin_synced_frame(stdout)?;
    queue!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
    crate::render::terminal_rows::paint_changed_rows(
        stdout,
        rows.saturating_sub(lines.len() as u16) / 2,
        usize::from(cols),
        &lines,
        None,
    )?;
    crate::config_tui::ui::end_synced_frame(stdout)?;
    Ok(())
}
