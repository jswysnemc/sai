use crate::config::AppConfig;
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::event::KeyCode;
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{self, Clear, ClearType};
use std::io::{self, Write};

use super::form::run_form;
use super::input::read_key;
use super::layout::full_frame;
use super::plugin_fields::{apply_plugin_fields, plugin_fields};
use super::theme::{selection_marks, BOLD, MUTED, RESET};
use super::ui::{display_width, draw_box, message, pad, truncate};

/// 编辑助手工具（含 Web 搜索）。
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
            KeyCode::Char(' ') => toggle_plugin(config, selected),
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
        let (_, name, description) = tools[index];
        let enabled = plugin_enabled(config, index);
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
    stdout.flush()?;
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

/// 返回助手工具菜单目录，下标与历史兼容配置索引一致（0 为 Web 搜索）。
///
/// 返回:
/// - 历史配置标识、显示名称和说明组成的固定目录
fn cli_tool_names() -> [(&'static str, &'static str, &'static str); 20] {
    [
        (
            "web",
            t("Web search", "Web 搜索"),
            t(
                "Web search backends and API credentials",
                "Web 搜索后端与 API 凭证",
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
            "image_generation",
            t("Image generation", "生图"),
            t("Generate images from text", "文本生成图片"),
        ),
        (
            "web_images",
            t("Web images", "搜图"),
            t(
                "Web image search, download and review",
                "网络图片搜索、下载与审核",
            ),
        ),
        (
            "print_image",
            t("Print image", "打印图片"),
            t("Terminal image print size", "终端图片打印尺寸"),
        ),
        (
            "memes",
            t("Memes", "表情包"),
            t("Persona meme library and send size", "人格表情库与发送尺寸"),
        ),
        (
            "knowledge_base",
            t("Knowledge base", "知识库"),
            t(
                "Local file retrieval and semantic index",
                "本地文件检索与语义索引",
            ),
        ),
        (
            "archlinux",
            "Arch Linux",
            t("AUR status and ArchWiki query", "AUR 状态与 ArchWiki 查询"),
        ),
        (
            "man",
            t("Online manuals", "在线手册"),
            t("Online man page search and read", "在线 man 手册搜索与读取"),
        ),
        (
            "memory",
            t("Memory", "记忆"),
            t("Long-term memory and association", "长期记忆与联想"),
        ),
        (
            "package_advisor",
            t("AUR review", "AUR 审查"),
            t("PKGBUILD/AUR security review", "PKGBUILD/AUR 安全审查"),
        ),
        (
            "linux_game_compatibility",
            t("Linux game compatibility", "Linux 游戏兼容"),
            t(
                "Proton/anti-cheat/compatibility query",
                "Proton/反作弊/兼容性查询",
            ),
        ),
        (
            "deep_diagnose",
            t("Deep diagnose", "深度诊断"),
            t("Multi-round diagnosis and review", "多轮诊断与审视修正"),
        ),
        (
            "diagnostics",
            t("System diagnostics", "系统诊断"),
            t(
                "Command limits for diagnostic tools",
                "诊断工具命令与输出限制",
            ),
        ),
        (
            "weather",
            t("Weather", "天气"),
            t("City weather and forecast query", "城市天气与预报查询"),
        ),
        (
            "exchange_rate",
            t("Exchange rate", "汇率"),
            t("Currency conversion and rate query", "货币换算与汇率查询"),
        ),
        (
            "calculator",
            t("Calculator", "计算器"),
            t("Scientific expression evaluation", "科学计算表达式求值"),
        ),
        (
            "hash_codec",
            t("Hash and codec", "哈希与编解码"),
            t("Hash digests and text encoding", "哈希摘要与文本编解码"),
        ),
        (
            "moegirl",
            t("Moegirl", "萌娘百科"),
            t("Moegirl encyclopedia query", "萌娘百科词条查询"),
        ),
        (
            "xuanxue",
            t("Zhouyi", "周易"),
            t("Zhouyi hexagram casting", "周易卦象起卦"),
        ),
    ]
}

/// 判断历史兼容配置索引对应的工具是否启用。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `index`: 历史兼容配置索引，0 表示 Web 搜索
///
/// 返回:
/// - 工具或 Web 搜索启用时返回 true
pub(super) fn plugin_enabled(config: &AppConfig, index: usize) -> bool {
    match index {
        0 => config.plugins.web.enabled,
        1 => config.plugins.vision.enabled,
        2 => config.plugins.image_generation.enabled,
        3 => config.plugins.web_images.enabled,
        4 => config.plugins.print_image.enabled,
        5 => config.plugins.memes.enabled,
        6 => config.plugins.knowledge_base.enabled,
        7 => config.plugins.archlinux.enabled,
        8 => config.plugins.man.enabled,
        9 => config.plugins.memory.enabled,
        10 => config.plugins.package_advisor.enabled,
        11 => config.plugins.linux_game_compatibility.enabled,
        12 => config.plugins.deep_diagnose.enabled,
        13 => config.plugins.diagnostics.enabled,
        14 => config.plugins.weather.enabled,
        15 => config.plugins.exchange_rate.enabled,
        16 => config.plugins.calculator.enabled,
        17 => config.plugins.hash_codec.enabled,
        18 => config.plugins.moegirl.enabled,
        19 => config.plugins.xuanxue.enabled,
        _ => false,
    }
}

/// 切换历史兼容配置索引对应的工具启用状态。
///
/// 参数:
/// - `config`: 待更新应用配置
/// - `index`: 历史兼容配置索引，0 表示 Web 搜索
///
/// 返回:
/// - 无返回值
pub(super) fn toggle_plugin(config: &mut AppConfig, index: usize) {
    let value = !plugin_enabled(config, index);
    match index {
        0 => config.plugins.web.enabled = value,
        1 => config.plugins.vision.enabled = value,
        2 => config.plugins.image_generation.enabled = value,
        3 => config.plugins.web_images.enabled = value,
        4 => config.plugins.print_image.enabled = value,
        5 => config.plugins.memes.enabled = value,
        6 => config.plugins.knowledge_base.enabled = value,
        7 => config.plugins.archlinux.enabled = value,
        8 => config.plugins.man.enabled = value,
        9 => config.plugins.memory.enabled = value,
        10 => config.plugins.package_advisor.enabled = value,
        11 => config.plugins.linux_game_compatibility.enabled = value,
        12 => config.plugins.deep_diagnose.enabled = value,
        13 => config.plugins.diagnostics.enabled = value,
        14 => config.plugins.weather.enabled = value,
        15 => config.plugins.exchange_rate.enabled = value,
        16 => config.plugins.calculator.enabled = value,
        17 => config.plugins.hash_codec.enabled = value,
        18 => config.plugins.moegirl.enabled = value,
        19 => config.plugins.xuanxue.enabled = value,
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
    let mut fields = plugin_fields(config, index);
    loop {
        if !run_form(stdout, &title, &mut fields)? {
            return Ok(());
        }
        // 解析失败时就地提示并重新打开表单，沿用已填内容
        match apply_plugin_fields(config, index, &fields) {
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

    /// 验证工具菜单以 Web 搜索开头，且菜单下标与配置索引一一对应。
    #[test]
    fn tool_menu_starts_with_web_search_and_maps_indices_directly() {
        let tools = cli_tool_names();
        let mut config = AppConfig::default();

        assert_eq!(tools.len(), 20);
        assert_eq!(tools[0].0, "web");
        assert_eq!(tools[1].0, "vision");
        // 菜单首行切换的正是 Web 搜索开关
        let before = config.plugins.web.enabled;
        toggle_plugin(&mut config, 0);
        assert_ne!(before, config.plugins.web.enabled);
    }

    /// 验证受开关控制的轻量工具都能在 TUI 中查看与切换。
    #[test]
    fn configurable_light_tools_are_reachable_from_menu() {
        let tools = cli_tool_names();
        let mut config = AppConfig::default();

        for id in [
            "weather",
            "exchange_rate",
            "calculator",
            "hash_codec",
            "moegirl",
            "xuanxue",
        ] {
            let position = tools
                .iter()
                .position(|(name, _, _)| *name == id)
                .unwrap_or_else(|| panic!("missing menu entry: {id}"));
            let before = plugin_enabled(&config, position);
            toggle_plugin(&mut config, position);
            assert_ne!(
                before,
                plugin_enabled(&config, position),
                "toggle had no effect: {id}"
            );
        }
    }
}
