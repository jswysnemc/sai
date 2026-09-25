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
use super::input::read_key;
use super::layout::full_frame;
use super::plugin_fields::{apply_plugin_fields, plugin_fields};
use super::theme::{selection_marks, BOLD, MUTED, RESET};
use super::ui::{display_width, draw_box, message, pad, truncate};

/// 编辑助手工具。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `config`: 待更新应用配置
///
/// 返回:
/// - 退出工具菜单或保存工具配置的结果
pub(crate) fn edit_cli_tools(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let mut selected = 0usize;
    loop {
        let count = cli_tool_names().len();
        draw_cli_tool_menu(stdout, config, selected)?;
        match read_key()? {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(count - 1),
            KeyCode::Char(' ') => toggle_plugin(config, cli_tool_names()[selected].0),
            KeyCode::Enter | KeyCode::Char('i') => edit_cli_tool_detail(stdout, config, selected)?,
            _ => {}
        }
    }
}

/// 绘制 CLI 助手工具列表。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `config`: 当前应用配置
/// - `selected`: 当前选中工具索引
///
/// 返回:
/// - 绘制与刷新结果
fn draw_cli_tool_menu(stdout: &mut io::Stdout, config: &AppConfig, selected: usize) -> Result<()> {
    let (cols, rows) = terminal::size()?;
    let frame = full_frame(cols, rows);
    let width = frame.width;
    let height = frame.height;
    let x = frame.x;
    let y = frame.y;
    super::ui::begin_synced_frame(stdout)?;
    queue!(stdout, Clear(ClearType::All))?;
    draw_box(stdout, x, y, width, height, t("TOOLS", "助手工具"))?;
    // 表头与数据行相同缩进（数据行有两列选中条前缀）
    queue!(
        stdout,
        MoveTo(x + 4, y + 1),
        Print(format!(
            "{MUTED}{BOLD}{}{RESET}",
            pad(
                &cli_tool_row(
                    t("State", "状态"),
                    t("Tool", "工具"),
                    t("Description", "说明"),
                    width.saturating_sub(6) as usize,
                ),
                width.saturating_sub(6) as usize,
            )
        ))
    )?;
    let tools = cli_tool_names();
    let visible_rows = height.saturating_sub(4) as usize;
    let start = selected.saturating_sub(visible_rows.saturating_sub(1));
    for row in 0..visible_rows {
        let index = start + row;
        if index >= tools.len() {
            break;
        }
        let (id, name, description) = tools[index];
        let enabled = plugin_enabled(config, id);
        let row_width = width.saturating_sub(6) as usize;
        queue!(stdout, MoveTo(x + 2, y + row as u16 + 2))?;
        let (bar, style) = selection_marks(index == selected);
        if index == selected {
            // 选中行：整行深底统一样式
            let state = if enabled {
                t("● on", "● 启用")
            } else {
                t("○ off", "○ 关闭")
            };
            let line = cli_tool_row(state, name, description, row_width);
            queue!(
                stdout,
                Print(format!("{bar}{style} {}{RESET}", pad(&line, row_width)))
            )?;
        } else {
            // 常规行：状态点分色，名称常规，说明弱化
            use super::theme::{DIM, OK};
            let (dot_style, state) = if enabled {
                (OK, t("● on", "● 启用"))
            } else {
                (DIM, t("○ off", "○ 关闭"))
            };
            let name_cell = pad(name, 24);
            let state_cell = pad(state, 8);
            let remaining = row_width
                .saturating_sub(display_width(&state_cell) + display_width(&name_cell))
                .max(10);
            queue!(
                stdout,
                Print(format!(
                    "{bar} {dot_style}{state_cell}{RESET}{name_cell}{MUTED}{}{RESET}",
                    truncate(description, remaining)
                ))
            )?;
        }
    }
    super::ui::draw_status_bar(
        stdout,
        &frame,
        &super::theme::help_line(&[
            ("Space", t("toggle", "开关")),
            ("Enter", t("configure", "配置")),
            ("↑↓", t("move", "移动")),
            ("q", t("back", "返回")),
        ]),
    )?;
    super::ui::end_synced_frame(stdout)?;
    Ok(())
}

/// 组装 CLI 助手工具列表行。
///
/// 参数:
/// - `state`: 启用状态
/// - `name`: 工具名称
/// - `description`: 工具说明
/// - `width`: 可用显示宽度
///
/// 返回:
/// - 已按宽度截断和补齐的列表行
fn cli_tool_row(state: &str, name: &str, description: &str, width: usize) -> String {
    let fixed = pad(state, 8) + &pad(name, 24);
    let remaining = width.saturating_sub(display_width(&fixed)).max(10);
    fixed + &truncate(description, remaining)
}

/// 返回助手工具菜单目录，以稳定配置标识绑定每个表单。
///
/// 返回:
/// - 历史配置标识、显示名称和说明组成的固定目录
fn cli_tool_names() -> [(&'static str, &'static str, &'static str); 3] {
    [
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
///
/// 参数:
/// - `config`: 当前应用配置
/// - `id`: 当前工具的稳定配置标识
///
/// 返回:
/// - 工具启用时返回 true
pub(super) fn plugin_enabled(config: &AppConfig, id: &str) -> bool {
    match id {
        "vision" => config.plugins.vision.enabled,
        "memory" => config.plugins.memory.enabled,
        "calculator" => config.plugins.calculator.enabled,
        _ => false,
    }
}

/// 切换配置标识对应的工具启用状态。
///
/// 参数:
/// - `config`: 待更新应用配置
/// - `id`: 当前工具的稳定配置标识
///
/// 返回:
/// - 无返回值
pub(super) fn toggle_plugin(config: &mut AppConfig, id: &str) {
    let value = !plugin_enabled(config, id);
    match id {
        "vision" => config.plugins.vision.enabled = value,
        "memory" => config.plugins.memory.enabled = value,
        "calculator" => config.plugins.calculator.enabled = value,
        _ => {}
    }
}

/// 编辑当前选中的 CLI 助手工具。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `config`: 待更新应用配置
/// - `index`: 工具菜单索引
///
/// 返回:
/// - 表单退出或配置保存结果
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
        // 解析失败时就地提示并重新打开表单，沿用已填内容
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
    use std::collections::BTreeSet;

    /// 【工具菜单测试】【稳定标识】菜单完整对应当前配置，切换只修改指定工具
    /// @returns 无；删除业务条目不能造成表单配置错位
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
}
