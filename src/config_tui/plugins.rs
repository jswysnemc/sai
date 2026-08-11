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

/// 编辑 CLI 助手可选工具。
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
            KeyCode::Char(' ') => toggle_plugin(config, cli_tool_config_index(selected)),
            KeyCode::Enter | KeyCode::Char('i') => edit_cli_tool_detail(stdout, config, selected)?,
            _ => {}
        }
    }
}

/// 编辑独立 Web 搜索配置。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `config`: 待更新应用配置
///
/// 返回:
/// - 表单退出或配置保存结果
pub(crate) fn edit_web_search(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    // 表单字段只构造一次，校验失败后沿用已填内容重新进入编辑
    let mut fields = plugin_fields(config, 0);
    loop {
        if !run_form(stdout, t(" WEB SEARCH ", " WEB 搜索 "), &mut fields)? {
            return Ok(());
        }
        match apply_plugin_fields(config, 0, &fields) {
            Ok(()) => return Ok(()),
            Err(err) => message(
                stdout,
                &format!("{}: {err}", t("Invalid input", "输入无效")),
            )?,
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
    draw_box(
        stdout,
        x,
        y,
        width,
        height,
        t(" CLI ASSISTANT TOOLS ", " CLI 助手工具 "),
    )?;
    queue!(
        stdout,
        MoveTo(x + 2, y + 1),
        Print(format!(
            "{MUTED}{}{RESET}",
            t(
                "Space toggle · Enter configure · ↑/↓ move · q back",
                "Space 开关 · Enter 配置 · ↑/↓ 移动 · q 返回",
            )
        ))
    )?;
    queue!(
        stdout,
        MoveTo(x + 2, y + 3),
        Print(format!(
            "{BOLD}{}{RESET}",
            pad(
                &cli_tool_row(
                    t("State", "状态"),
                    t("Tool", "工具"),
                    t("Description", "说明"),
                    width.saturating_sub(4) as usize,
                ),
                width.saturating_sub(4) as usize,
            )
        ))
    )?;
    let tools = cli_tool_names();
    let visible_rows = height.saturating_sub(6) as usize;
    let start = selected.saturating_sub(visible_rows.saturating_sub(1));
    for row in 0..visible_rows {
        let index = start + row;
        if index >= tools.len() {
            break;
        }
        let (_, name, description) = tools[index];
        let state = if plugin_enabled(config, cli_tool_config_index(index)) {
            "[ON]"
        } else {
            "[OFF]"
        };
        let line = cli_tool_row(state, name, description, width.saturating_sub(6) as usize);
        queue!(stdout, MoveTo(x + 2, y + row as u16 + 4))?;
        let (bar, style) = selection_marks(index == selected);
        queue!(
            stdout,
            Print(format!(
                "{bar} {style}{}{RESET}",
                pad(&line, width.saturating_sub(6) as usize)
            ))
        )?;
    }
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

/// 返回 CLI 助手工具菜单目录。
///
/// 返回:
/// - 历史配置标识、显示名称和说明组成的固定目录
fn cli_tool_names() -> [(&'static str, &'static str, &'static str); 19] {
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
    let title = format!(
        " {}: {} ",
        t("CLI TOOL", "CLI 工具"),
        cli_tool_names()[index].1
    );
    let config_index = cli_tool_config_index(index);
    let mut fields = plugin_fields(config, config_index);
    if !run_form(stdout, &title, &mut fields)? {
        return Ok(());
    }
    apply_plugin_fields(config, config_index, &fields)
}

/// 将工具菜单索引映射到历史兼容配置索引。
///
/// 参数:
/// - `index`: 不含 Web 搜索的工具菜单索引
///
/// 返回:
/// - plugin_fields 使用的历史兼容配置索引
fn cli_tool_config_index(index: usize) -> usize {
    index + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 CLI 助手工具菜单不再包含 Web 搜索，并保持历史配置索引映射。
    #[test]
    fn cli_tool_menu_excludes_web_search_and_maps_config_indices() {
        let tools = cli_tool_names();

        assert_eq!(tools.len(), 19);
        assert_eq!(tools[0].0, "vision");
        assert!(tools.iter().all(|(id, _, _)| *id != "web"));
        assert_eq!(cli_tool_config_index(0), 1);
        assert_eq!(cli_tool_config_index(tools.len() - 1), 19);
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
            let config_index = cli_tool_config_index(position);
            let before = plugin_enabled(&config, config_index);
            toggle_plugin(&mut config, config_index);
            assert_ne!(
                before,
                plugin_enabled(&config, config_index),
                "toggle had no effect: {id}"
            );
        }
    }
}
