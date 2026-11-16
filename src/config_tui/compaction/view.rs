//! 压缩策略卡片：模式选择、实时阈值和快捷键说明。

use super::card::{self, ACCENT, BORDER, BRAND, MUTED, RESET, SELECTED};
use super::draft::{format_percent, Draft, Outcome, RESERVE_PRESETS};
use crate::config_tui::mouse::{ClickTracker, HitButtons, HitRect};
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use crossterm::{
    cursor::MoveTo,
    queue,
    terminal::{self, Clear, ClearType},
};
use std::io;

/// 【上下文】【鼠标热区】记录策略卡片的模式按钮、档位及底栏操作坐标。
#[derive(Clone, Debug, Default)]
pub(super) struct CardHitbox {
    pub card: HitRect,
    pub mode0: HitRect,
    pub mode1: HitRect,
    pub actions: HitButtons<KeyCode>,
}

impl CardHitbox {
    /// 【上下文】【热区计算】按卡片起始行与列宽同步生成可点区域。
    ///
    /// 参数:
    /// - `draft`: 当前策略草稿
    /// - `top`: 卡片首行屏幕坐标
    /// - `cols`: 终端列宽
    /// - `height`: 卡片总行数
    ///
    /// 返回:
    /// - 与当前绘制帧对齐的热区集合
    pub(super) fn compute(draft: &Draft, top: u16, cols: usize, height: usize) -> Self {
        let width = cols.clamp(1, 88);
        let framed = width >= 16;
        let inner = width.saturating_sub(if framed { 4 } else { 0 }).max(1);
        let left_x = if framed { 2u16 } else { 0u16 };
        let choices = selectors(draft);
        let w0 = crate::render::ansi_clip::ansi_visible_width(&choices[0]);
        let w1 = crate::render::ansi_clip::ansi_visible_width(&choices[1]);
        let combined = choices.join("   ");
        let stacked = crate::render::ansi_clip::ansi_visible_width(&combined) > inner;
        let mode0 = HitRect {
            x: left_x,
            y: top.saturating_add(1),
            width: w0 as u16,
            height: 1,
        };
        let mode1 = if stacked {
            HitRect {
                x: left_x,
                y: top.saturating_add(2),
                width: w1.min(inner) as u16,
                height: 1,
            }
        } else {
            HitRect {
                x: left_x.saturating_add(w0 as u16).saturating_add(3),
                y: top.saturating_add(1),
                width: w1 as u16,
                height: 1,
            }
        };

        let mut actions = HitButtons::default();
        if height > 0 && draft.input.is_none() {
            let help_str = help_text(draft);
            if framed && crate::render::ansi_clip::ansi_visible_width(help_str) + 6 <= width {
                let mut col = 3usize;
                let bar_y = top.saturating_add(height.saturating_sub(1) as u16);
                for seg in help_str.split(" · ") {
                    let seg_w = crate::render::ansi_clip::ansi_visible_width(seg);
                    let mapped = match seg.split_whitespace().next() {
                        Some("Enter") => Some(KeyCode::Enter),
                        Some("Esc") => Some(KeyCode::Esc),
                        Some("r") => Some(KeyCode::Char('r')),
                        Some("e") => Some(KeyCode::Char('e')),
                        Some("Tab") => Some(KeyCode::Tab),
                        _ => None,
                    };
                    if let Some(code) = mapped {
                        actions.push(
                            HitRect {
                                x: col as u16,
                                y: bar_y,
                                width: seg_w as u16,
                                height: 1,
                            },
                            code,
                        );
                    }
                    col = col.saturating_add(seg_w).saturating_add(3);
                }
            }
        }
        Self {
            card: HitRect {
                x: 0,
                y: top,
                width: width as u16,
                height: height as u16,
            },
            mode0,
            mode1,
            actions,
        }
    }
}

/// 生成帮助条文案，保证渲染与命中区共用同一文案定义。
fn help_text(draft: &Draft) -> &'static str {
    if draft.input.is_some() {
        t(
            "Enter apply · Esc undo · Ctrl+A select · Ctrl+U clear",
            "Enter 确认 · Esc 撤销 · Ctrl+A 全选 · Ctrl+U 清空",
        )
    } else {
        t(
            "Tab mode · ←→ adjust · 0–3 presets · e edit · r reset · Enter save · Esc cancel",
            "Tab 切换 · ←→ 调节 · 0–3 档位 · e 输入 · r 恢复 · Enter 保存 · Esc 取消",
        )
    }
}

/// 压缩卡片单次鼠标点击区域枚举。
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum CardClick {
    Mode(usize),
    Action(KeyCode),
}

/// 【上下文】【鼠标处理】响应模式切换、数值滚轮步进及底栏保存/取消点击。
///
/// 参数:
/// - `mouse`: 鼠标输入事件
/// - `hitbox`: 当前帧卡片热区
/// - `draft`: 待更新策略草稿
/// - `clicks`: 单击判定追踪器
///
/// 返回:
/// - `(是否需要重绘, 可选结束动作)`
pub(super) fn apply_mouse(
    mouse: MouseEvent,
    hitbox: &CardHitbox,
    draft: &mut Draft,
    hovered: &mut Option<CardClick>,
    clicks: &mut ClickTracker<CardClick>,
) -> (bool, Option<Outcome>) {
    let target = if let Some(code) = hitbox.actions.hit(mouse.column, mouse.row) {
        Some(CardClick::Action(code))
    } else if hitbox.mode0.contains(mouse.column, mouse.row) {
        Some(CardClick::Mode(0))
    } else if hitbox.mode1.contains(mouse.column, mouse.row) {
        Some(CardClick::Mode(1))
    } else {
        None
    };
    if matches!(mouse.kind, MouseEventKind::Moved) {
        if target != *hovered {
            *hovered = target;
            return (true, None);
        }
        return (false, None);
    }
    if hitbox.card.contains(mouse.column, mouse.row) && draft.input.is_none() {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                let out = draft.handle(KeyCode::Right);
                return (true, out);
            }
            MouseEventKind::ScrollDown => {
                let out = draft.handle(KeyCode::Left);
                return (true, out);
            }
            _ => {}
        }
    }
    let Some(clicked) = clicks.update(mouse, target) else {
        return (false, None);
    };
    *hovered = None;
    match clicked {
        CardClick::Mode(idx) => {
            if draft.input.is_none() {
                if draft.selected != idx {
                    draft.selected = idx;
                    draft.reset = false;
                } else {
                    let _ = draft.handle(KeyCode::Right);
                }
                return (true, None);
            }
        }
        CardClick::Action(code) => {
            let out = draft.handle_event(KeyEvent::new(code, KeyModifiers::NONE));
            return (true, out);
        }
    }
    (false, None)
}

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

/// 【上下文】【模式高亮】为未选中却被鼠标悬停的模式控件叠加提亮底色与主题前景色。
fn selectors_interactive(draft: &Draft, hovered: Option<CardClick>) -> [String; 2] {
    let mut base = selectors(draft);
    for (index, item) in base.iter_mut().enumerate() {
        if hovered == Some(CardClick::Mode(index)) && index != draft.selected {
            *item = format!(
                "\x1b[48;2;28;48;54m{ACCENT}{}{RESET}",
                crate::render::ansi_clip::strip_ansi(item)
            );
        }
    }
    base
}

/// 标亮底栏操作提示中被鼠标悬停的分段。
fn highlight_card_help(help_str: &str, hovered: Option<CardClick>) -> String {
    let Some(CardClick::Action(target)) = hovered else {
        return help_str.to_string();
    };
    help_str
        .split(" · ")
        .map(|seg| {
            let matched = match seg.split_whitespace().next() {
                Some("Enter") => target == KeyCode::Enter,
                Some("Esc") => target == KeyCode::Esc,
                Some("r") => target == KeyCode::Char('r'),
                Some("e") => target == KeyCode::Char('e'),
                Some("Tab") => target == KeyCode::Tab,
                _ => false,
            };
            if matched {
                format!("{ACCENT}\x1b[1m{seg}{RESET}{MUTED}")
            } else {
                seg.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// 【上下文】【卡片排版】宽屏保持五行，窄屏拆分控件和统计，不强制最小宽度。
/// 参数：草稿、上下文与终端列数；返回：完整 ANSI 卡片。
#[cfg(test)]
pub(super) fn frame(draft: &Draft, context: &PreviewContext, cols: usize) -> Vec<String> {
    frame_interactive(draft, context, None, cols)
}

/// 【上下文】【悬停卡片排版】在标准布局上高亮鼠标悬停的模式切换按钮与底栏快捷键。
pub(super) fn frame_interactive(
    draft: &Draft,
    context: &PreviewContext,
    hovered: Option<CardClick>,
    cols: usize,
) -> Vec<String> {
    let width = cols.clamp(1, 88);
    let inner = width.saturating_sub(4).max(1);
    let preview = draft.preview();
    let policy = preview.as_ref().copied().unwrap_or(draft.policy);
    let choices = selectors_interactive(draft, hovered);
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
    let help = highlight_card_help(help_text(draft), hovered);
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
    card::frame(&title, &body, &help, width)
}

/// 【上下文】【全局绘制】在配置界面自身画布中居中显示同一张策略卡片。
/// 参数：输出、草稿与上下文；返回：绘制结果与鼠标热区。
pub(super) fn draw(
    stdout: &mut io::Stdout,
    draft: &Draft,
    context: &PreviewContext,
    hovered: Option<CardClick>,
) -> Result<CardHitbox> {
    let (cols, rows) = terminal::size()?;
    let lines = card::fit_height(
        frame_interactive(draft, context, hovered, usize::from(cols)),
        usize::from(rows),
    );
    let top = rows.saturating_sub(lines.len() as u16) / 2;
    crate::config_tui::ui::begin_synced_frame(stdout)?;
    queue!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
    crate::render::terminal_rows::paint_changed_rows(stdout, top, usize::from(cols), &lines, None)?;
    crate::config_tui::ui::end_synced_frame(stdout)?;
    Ok(CardHitbox::compute(
        draft,
        top,
        usize::from(cols),
        lines.len(),
    ))
}
