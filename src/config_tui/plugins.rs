//! 内置助手工具配置列表：支持单键启停、连续整行选中/悬停底色与明细表单跳转。

use crate::config::AppConfig;
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::event::KeyCode;
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{self, Clear, ClearType};
use std::io;

use super::form::run_form;
use super::input::{read_event, ConfigEvent};
use super::layout::full_frame;
use super::mouse::{HitRect, ListHitbox, ListMouseOutcome, ListMouseState};
use super::plugin_fields::{apply_plugin_fields, plugin_fields};
use super::theme::{row_marks, tint_bg, ThemePalette, BOLD, RESET};
use super::ui::{
    display_width, draw_box, draw_scroll_indicator, draw_status_bar_interactive, message, pad,
    status_bar_hits, truncate, MenuHitbox,
};

/// 【助手工具】【菜单循环】编辑内置 CLI 助手工具启停与参数明细。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `config`: 待更新应用配置
///
/// 返回:
/// - 退出工具菜单或保存工具配置的结果
pub(crate) fn edit_cli_tools(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let mut selected = 0usize;
    let mut mouse = ListMouseState::default();
    loop {
        let count = cli_tool_names().len();
        let hitbox = draw_cli_tool_menu(stdout, config, selected, &mouse)?;
        let code = match read_event()? {
            ConfigEvent::Resize | ConfigEvent::Paste(_) => continue,
            ConfigEvent::Key(key) => key.code,
            ConfigEvent::Mouse(event) => {
                let mut footer_redraw = false;
                if matches!(event.kind, crossterm::event::MouseEventKind::Moved) {
                    let next_footer = hitbox.footer.rect_at(event.column, event.row);
                    if next_footer != mouse.footer_hover {
                        mouse.footer_hover = next_footer;
                        footer_redraw = true;
                    }
                }
                if let Some(footer_code) = mouse.track_buttons(event, &hitbox.footer) {
                    mouse.footer_hover = None;
                    footer_code
                } else {
                    match mouse.handle(event, &hitbox.list, selected, false) {
                        ListMouseOutcome::None if footer_redraw => continue,
                        ListMouseOutcome::None => continue,
                        ListMouseOutcome::Redraw => continue,
                        ListMouseOutcome::RightClick(idx) => {
                            selected = idx;
                            KeyCode::Char(' ')
                        }
                        ListMouseOutcome::Select(idx) => {
                            selected = idx;
                            if event.column < hitbox.list.list.x.saturating_add(12) {
                                KeyCode::Char(' ')
                            } else {
                                continue;
                            }
                        }
                        ListMouseOutcome::Activate(idx) => {
                            selected = idx;
                            if event.column < hitbox.list.list.x.saturating_add(12) {
                                KeyCode::Char(' ')
                            } else {
                                KeyCode::Enter
                            }
                        }
                    }
                }
            }
        };
        match code {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(count - 1),
            KeyCode::Char(' ') => toggle_plugin(config, cli_tool_names()[selected].0),
            KeyCode::Enter | KeyCode::Char('i') => edit_cli_tool_detail(stdout, config, selected)?,
            _ => {}
        }
    }
}

/// 【助手工具】【行渲染】构造工具列表单行文本，修复选中态与悬停态因中间 RESET 导致的底色断层。
///
/// 参数:
/// - `enabled`: 工具是否启用
/// - `name`: 工具显示名称
/// - `description`: 工具用途说明
/// - `row_width`: 可用显示列宽（不含左侧两列前缀）
/// - `is_selected`: 是否处于键盘选中态
/// - `is_hovered`: 是否处于鼠标悬停态
///
/// 返回:
/// - 带完整连续背景或分色样式的单行 ANSI 文本
fn format_plugin_row(
    enabled: bool,
    name: &str,
    description: &str,
    row_width: usize,
    is_selected: bool,
    is_hovered: bool,
) -> String {
    let palette = ThemePalette::current();
    let (bar, style) = row_marks(is_selected, is_hovered);
    if is_selected {
        let state = if enabled {
            t("● on", "● 启用")
        } else {
            t("○ off", "○ 关闭")
        };
        let line = cli_tool_row(state, name, description, row_width);
        let raw = format!("{bar}{style} {}{RESET}", pad(&line, row_width));
        return tint_bg(&raw, palette.select_bg);
    }
    let (dot_style, state) = if enabled {
        (palette.ok, t("● on", "● 启用"))
    } else {
        (palette.dim, t("○ off", "○ 关闭"))
    };
    let state_w = 8.min(row_width);
    let name_w = 24.min(row_width.saturating_sub(state_w));
    let state_cell = pad(state, state_w);
    let name_cell = pad(name, name_w);
    let desc_w = row_width.saturating_sub(display_width(&state_cell) + display_width(&name_cell));
    let desc_cell = pad(&truncate(description, desc_w), desc_w);
    let label_style = if is_hovered { style } else { RESET };
    let raw = format!(
        "{bar} {dot_style}{state_cell}{RESET}{label_style}{name_cell}{}{desc_cell}{RESET}",
        palette.muted
    );
    if is_hovered {
        // 1. 【助手工具】【连续悬停】对状态列、名称列与说明列统一补回悬停底色，消除色块断层
        tint_bg(&raw, palette.hover_bg)
    } else {
        raw
    }
}

/// 【助手工具】【界面绘制】绘制 CLI 助手工具列表并返回鼠标命中区。
fn draw_cli_tool_menu(
    stdout: &mut io::Stdout,
    config: &AppConfig,
    selected: usize,
    mouse: &ListMouseState,
) -> Result<MenuHitbox> {
    let palette = ThemePalette::current();
    let (cols, rows) = terminal::size()?;
    let frame = full_frame(cols, rows);
    let width = frame.width;
    let height = frame.height;
    let x = frame.x;
    let y = frame.y;
    super::ui::begin_synced_frame(stdout)?;
    queue!(stdout, Clear(ClearType::All))?;
    draw_box(stdout, x, y, width, height, t("TOOLS", "助手工具"))?;
    let header_w = width.saturating_sub(6) as usize;
    queue!(
        stdout,
        MoveTo(x + 4, y + 1),
        Print(crate::render::readable_terminal_text(
            &(format!(
                "{}{BOLD}{}{RESET}",
                palette.muted,
                pad(
                    &cli_tool_row(
                        t("State", "状态"),
                        t("Tool", "工具"),
                        t("Description", "说明"),
                        header_w,
                    ),
                    header_w,
                )
            ))
            .to_string()
        ))
    )?;
    let tools = cli_tool_names();
    let visible_rows = height.saturating_sub(4).max(1) as usize;
    let start = selected.saturating_sub(visible_rows.saturating_sub(1));
    let has_scroll = tools.len() > visible_rows;
    let row_width = width.saturating_sub(if has_scroll { 7 } else { 6 }) as usize;
    for row in 0..visible_rows {
        let index = start + row;
        if index >= tools.len() {
            break;
        }
        let (id, name, description) = tools[index];
        let enabled = plugin_enabled(config, id);
        queue!(stdout, MoveTo(x + 2, y + row as u16 + 2))?;
        let rendered = format_plugin_row(
            enabled,
            name,
            description,
            row_width,
            index == selected,
            mouse.hovered == Some(index),
        );
        queue!(
            stdout,
            Print(crate::render::readable_terminal_text(&rendered))
        )?;
    }
    draw_scroll_indicator(
        stdout,
        x.saturating_add(width.saturating_sub(3)),
        y.saturating_add(2),
        visible_rows as u16,
        tools.len(),
        start,
        visible_rows,
    )?;
    let help = super::theme::help_line(&[
        ("Space", t("toggle", "开关")),
        ("Enter", t("configure", "配置")),
        ("↑↓", t("move", "移动")),
        ("q", t("back", "返回")),
    ]);
    draw_status_bar_interactive(stdout, &frame, &help, mouse.footer_hover)?;
    let footer = status_bar_hits(&frame, &help);
    super::ui::end_synced_frame(stdout)?;
    Ok(MenuHitbox {
        list: ListHitbox {
            frame: HitRect::from(frame),
            list: HitRect {
                x: x.saturating_add(2),
                y: y.saturating_add(2),
                width: width.saturating_sub(4),
                height: visible_rows as u16,
            },
            start,
            total: tools.len(),
            visible_rows,
        },
        footer,
    })
}

/// 【助手工具】【列对齐】组装 CLI 助手工具列表纯文本行。
fn cli_tool_row(state: &str, name: &str, description: &str, width: usize) -> String {
    let state_w = 8.min(width);
    let name_w = 24.min(width.saturating_sub(state_w));
    let fixed = pad(state, state_w) + &pad(name, name_w);
    let remaining = width.saturating_sub(display_width(&fixed));
    fixed + &truncate(description, remaining)
}

/// 返回助手工具菜单目录，以稳定配置标识绑定每个表单。
fn cli_tool_names() -> [(&'static str, &'static str, &'static str); 4] {
    [
        (
            "web",
            t("Web search", "网页搜索"),
            t(
                "Built-in search providers and fallback",
                "内置搜索供应商与自动回退",
            ),
        ),
        (
            "vision",
            t("Vision", "识图"),
            t(
                "Image understanding and terminal preview",
                "图片理解和终端预览",
            ),
        ),
        (
            "memory",
            t("Memory", "记忆"),
            t("Long-term memory and association", "长期记忆与联想"),
        ),
        (
            "calculator",
            t("Calculator", "计算器"),
            t("Scientific expression evaluation", "科学计算表达式求值"),
        ),
    ]
}

/// 判断配置标识对应的工具是否启用。
pub(super) fn plugin_enabled(config: &AppConfig, id: &str) -> bool {
    match id {
        "web" => config.plugins.web.enabled,
        "vision" => config.plugins.vision.enabled,
        "memory" => config.plugins.memory.enabled,
        "calculator" => config.plugins.calculator.enabled,
        _ => false,
    }
}

/// 切换配置标识对应的工具启用状态。
pub(super) fn toggle_plugin(config: &mut AppConfig, id: &str) {
    let value = !plugin_enabled(config, id);
    match id {
        "web" => config.plugins.web.enabled = value,
        "vision" => config.plugins.vision.enabled = value,
        "memory" => config.plugins.memory.enabled = value,
        "calculator" => config.plugins.calculator.enabled = value,
        _ => {}
    }
}

/// 编辑当前选中的 CLI 助手工具。
fn edit_cli_tool_detail(
    stdout: &mut io::Stdout,
    config: &mut AppConfig,
    index: usize,
) -> Result<()> {
    let title = format!(" {}: {} ", t("TOOL", "工具"), cli_tool_names()[index].1);
    let id = cli_tool_names()[index].0;
    let mut fields = plugin_fields(config, id);
    loop {
        if !run_form(stdout, &title, &mut fields)? {
            return Ok(());
        }
        match apply_plugin_fields(config, id, &fields) {
            Ok(()) => return Ok(()),
            Err(err) => message(
                stdout,
                &format!("{}: {err}", t("Invalid input", "输入无效")),
            )?,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_tui::theme::SELECTION_BAR;
    use crate::config_tui::ui::strip_ansi_plain;
    use std::collections::BTreeSet;

    /// 【工具菜单测试】【稳定标识】菜单完整对应当前配置，切换只修改指定工具
    #[test]
    fn tool_menu_uses_stable_ids_for_current_configuration() {
        let mut config = AppConfig::default();
        let tools = cli_tool_names();
        let serialized = serde_json::to_value(&config.plugins).unwrap();
        let expected = serialized
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            tools.iter().map(|entry| entry.0).collect::<BTreeSet<_>>(),
            expected
        );
        for (id, _, _) in tools {
            let before = serde_json::to_value(&config.plugins).unwrap();
            toggle_plugin(&mut config, id);
            let after = serde_json::to_value(&config.plugins).unwrap();
            for key in before.as_object().unwrap().keys() {
                if key == id {
                    assert_ne!(before[key]["enabled"], after[key]["enabled"]);
                } else {
                    assert_eq!(before[key], after[key]);
                }
            }
        }
    }

    /// 【助手工具测试】【连续悬停背景】验证悬停行不含异色双竖条且背景在状态列后连续不断开。
    #[test]
    fn plugin_hover_row_fills_continuous_background() {
        let palette = ThemePalette::current();
        let hovered = format_plugin_row(true, "Web search", "Built-in search", 64, false, true);
        assert!(!strip_ansi_plain(&hovered).contains(SELECTION_BAR));
        let body = hovered.strip_suffix(RESET).unwrap();
        assert!(!body.contains(&format!("{RESET} ")));
        assert!(body.contains(&format!("{RESET}{}", palette.hover_bg)));
    }
}
