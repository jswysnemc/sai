use super::draft::{format_percent, format_tokens, Draft};
use crate::config_tui::{layout::FrameRect, theme, ui};
use crate::i18n::text as t;
use crate::render::fold_text::wrap_display_lines;
use crate::state::CompactionBudgetPolicy;
use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    queue,
    style::Print,
    terminal::{self, Clear, ClearType},
};
use std::io;

/// 【上下文】【策略预览】展示当前窗口与配置作用范围
pub(super) struct PreviewContext {
    pub window: usize,
    pub used: Option<usize>,
    pub scope: String,
    pub session: bool,
}

/// 【上下文】【策略预览】生成可折行的面板内容
/// 参数: draft 为编辑草稿，context 为窗口信息，width 为正文宽度
/// 返回: 文本行及选中行下标
pub(super) fn content(
    draft: &Draft,
    context: &PreviewContext,
    width: usize,
) -> (Vec<String>, usize) {
    let preview = draft.preview();
    let policy = preview.as_ref().copied().unwrap_or(draft.policy);
    let trigger = policy.trigger_chars(context.window);
    let ratio_trigger =
        CompactionBudgetPolicy::from_context(policy.ratio, 0).trigger_chars(context.window);
    let reserve_trigger = (policy.reserve_tokens > 0 && policy.reserve_tokens < context.window)
        .then(|| context.window - policy.reserve_tokens);
    let reserve_active = reserve_trigger.is_some_and(|value| value > ratio_trigger);
    let active = t("in effect", "生效");
    let mut blocks = vec![context.scope.clone()];
    if context.window == 0 {
        blocks.push(
            t(
                "Window unknown; preview unavailable",
                "窗口未知，暂时无法计算触发量",
            )
            .to_string(),
        );
    } else if preview.is_err() {
        blocks.push(
            t(
                "Invalid draft; preview unavailable",
                "输入无效，暂时无法预览",
            )
            .to_string(),
        );
    } else {
        let percent = trigger as f64 * 100.0 / context.window as f64;
        if width < 50 {
            blocks.push(format!(
                "{} {} ({percent:.1}%)",
                t("Compacts at", "触发于"),
                format_tokens(trigger)
            ));
            blocks.push(format!(
                "{} {} tokens",
                t("Window", "窗口"),
                format_tokens(context.window)
            ));
        } else {
            blocks.push(format!(
                "{} {} / {} tokens ({percent:.1}%)",
                t("Compacts at", "触发于"),
                format_tokens(trigger),
                format_tokens(context.window)
            ));
        }
        blocks.push(progress_bar(
            context.used.unwrap_or(0),
            trigger,
            context.window,
            width.min(60),
        ));
        blocks.push(if let Some(used) = context.used {
            format!(
                "{} {} · {} {}",
                t("Used", "已用"),
                format_tokens(used),
                t("Until trigger", "距触发"),
                format_tokens(trigger.saturating_sub(used))
            )
        } else {
            t(
                "| trigger · preview uses the active model window",
                "| 触发位置 · 按当前模型窗口预览",
            )
            .to_string()
        });
    }
    blocks.push(
        t(
            "Both conditions use the later trigger.",
            "两个条件取更晚到达的触发点。",
        )
        .to_string(),
    );
    blocks.push(String::new());
    let ratio_value = if draft.selected == 0 {
        draft.input.as_deref()
    } else {
        None
    }
    .map(|value| format!("{value}_"))
    .unwrap_or_else(|| format!("{}%", format_percent(policy.ratio)));
    let reserve_value = if draft.selected == 1 {
        draft.input.as_deref()
    } else {
        None
    }
    .map(|value| format!("{value}_"))
    .unwrap_or_else(|| format!("{} tokens", format_tokens(policy.reserve_tokens)));
    let mut selected_block = 0;
    // 1. 【上下文】【策略预览】两个条件都显示换算值，并标记实际决定阈值的条件
    for (index, label) in [
        format!("{}: {ratio_value}", t("Usage ratio", "占用比例")),
        format!("{}: {reserve_value}", t("Reserved tokens", "预留数量")),
    ]
    .into_iter()
    .enumerate()
    {
        if draft.selected == index {
            selected_block = blocks.len();
        }
        blocks.push(format!(
            "{} {label}",
            if draft.selected == index { ">" } else { " " }
        ));
        let condition = if index == 0 {
            Some(ratio_trigger)
        } else {
            reserve_trigger
        };
        blocks.push(
            match condition.filter(|_| context.window > 0 && preview.is_ok()) {
                Some(value) => format!(
                    "  {} {} {}",
                    t("Trigger", "触发量"),
                    format_tokens(value),
                    if reserve_active == (index == 1) {
                        active
                    } else {
                        ""
                    }
                ),
                None => format!("  {}", t("Not applied", "不参与")),
            },
        );
    }
    blocks.push(
        t(
            "Reserve presets: 0 off / 1 8k / 2 50k / 3 100k",
            "预留快捷档：0 关闭 / 1 8k / 2 50k / 3 100k",
        )
        .to_string(),
    );
    if policy.reserve_tokens >= context.window && context.window > 0 {
        blocks.push(
            t(
                "Reserve reaches/exceeds window; ratio only.",
                "预留达到或超过窗口，仅按比例触发。",
            )
            .to_string(),
        );
    }
    blocks.push(String::new());
    // 2. 【上下文】【策略预览】保存、取消和恢复操作与现有配置界面保持一致
    for (index, label) in [
        if context.session {
            t("Restore workspace defaults", "恢复全局默认")
        } else {
            t("Restore built-in defaults", "恢复内置默认")
        },
        t("Save", "保存"),
        t("Cancel", "取消"),
    ]
    .into_iter()
    .enumerate()
    {
        let selected = draft.selected == index + 2;
        if selected {
            selected_block = blocks.len();
        }
        blocks.push(format!("{} {label}", if selected { ">" } else { " " }));
    }
    if draft.reset {
        blocks.push(
            t(
                "Defaults restored in draft; save to apply.",
                "默认值已恢复到草稿，保存后生效。",
            )
            .to_string(),
        );
    }
    if let Some(error) = draft
        .error
        .clone()
        .or_else(|| preview.err().map(|error| error.to_string()))
    {
        blocks.push(error);
    }
    let mut lines = Vec::new();
    let mut selected_line = 0;
    for (index, block) in blocks.iter().enumerate() {
        if index == selected_block {
            selected_line = lines.len();
        }
        lines.extend(wrap_display_lines(block, width.max(1)));
    }
    (lines, selected_line)
}

/// 【上下文】【策略预览】绘制占用条与触发标记
/// 参数: used 为占用，trigger 为触发量，window 为窗口，width 为可用列数
/// 返回: 不超过指定列宽的字符进度条
fn progress_bar(used: usize, trigger: usize, window: usize, width: usize) -> String {
    let size = width.saturating_sub(2).max(1);
    let filled = ((used.min(window) as f64 / window as f64) * size as f64) as usize;
    let mark = (((trigger as f64 / window as f64) * size as f64) as usize).min(size - 1);
    let body: String = (0..size)
        .map(|index| {
            if index == mark {
                '|'
            } else if index < filled {
                '='
            } else {
                '·'
            }
        })
        .collect();
    format!("[{body}]")
}

/// 【上下文】【面板绘制】使用统一边框和主题绘制可滚动的紧凑面板
/// 参数: stdout 为终端输出，draft 为草稿，context 为预览信息
/// 返回: 绘制结果
pub(super) fn draw(stdout: &mut io::Stdout, draft: &Draft, context: &PreviewContext) -> Result<()> {
    let (cols, rows) = terminal::size()?;
    if cols < 16 || rows < 6 {
        ui::begin_synced_frame(stdout)?;
        queue!(
            stdout,
            Clear(ClearType::All),
            MoveTo(0, 0),
            Print(ui::truncate(
                t("Resize terminal", "请扩大窗口"),
                cols.saturating_sub(1) as usize
            ))
        )?;
        ui::end_synced_frame(stdout)?;
        return Ok(());
    }
    let width = cols.saturating_sub(2).clamp(1, 86);
    let inner = width.saturating_sub(4).max(1) as usize;
    let (lines, selected) = content(draft, context, inner);
    let height = ((lines.len() + 4) as u16).min(rows).max(1);
    let frame = FrameRect {
        x: cols.saturating_sub(width) / 2,
        y: rows.saturating_sub(height) / 2,
        width,
        height,
    };
    ui::begin_synced_frame(stdout)?;
    queue!(stdout, Clear(ClearType::All))?;
    ui::draw_box(
        stdout,
        frame.x,
        frame.y,
        width,
        height,
        &ui::truncate(
            t(" AUTO-COMPACT ", " 自动压缩 "),
            width.saturating_sub(8) as usize,
        ),
    )?;
    let body_height = height.saturating_sub(3) as usize;
    let start = if body_height == 0 {
        0
    } else {
        selected.saturating_sub(body_height - 1)
    };
    for (index, line) in lines.iter().enumerate().skip(start).take(body_height) {
        let style = if index == selected {
            theme::ACCENT
        } else {
            theme::VALUE
        };
        queue!(
            stdout,
            MoveTo(frame.x + 2, frame.y + 1 + (index - start) as u16),
            Print(format!(
                "{style}{}{reset}",
                ui::truncate(line, inner),
                reset = theme::RESET
            ))
        )?;
    }
    let help = if draft.input.is_some() {
        theme::help_line(&[
            ("Enter", t("apply", "确认")),
            ("Esc", t("undo", "撤销")),
            ("Del", t("clear", "清空")),
        ])
    } else if width < 60 {
        format!(
            "{}↑↓/←→ Enter{} s {} q {}",
            theme::ACCENT,
            theme::RESET,
            t("save", "保存"),
            t("cancel", "取消")
        )
    } else {
        theme::help_line(&[
            ("↑↓", t("select", "选择")),
            ("←→", t("adjust", "调整")),
            ("Enter", t("edit", "输入")),
            ("s", t("save", "保存")),
            ("q", t("cancel", "取消")),
        ])
    };
    ui::draw_status_bar(stdout, &frame, &help)?;
    ui::end_synced_frame(stdout)?;
    Ok(())
}
