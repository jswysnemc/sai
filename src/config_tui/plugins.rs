use crate::config::AppConfig;
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::event::KeyCode;
use crossterm::queue;
use crossterm::style::{Attribute, Print, SetAttribute};
use crossterm::terminal::{self, Clear, ClearType};
use std::io::{self, Write};

use super::form::run_form;
use super::input::read_key;
use super::plugin_fields::{apply_plugin_fields, plugin_fields};
use super::ui::{display_width, draw_box, pad, truncate};

pub(crate) fn edit_plugins(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let mut selected = 0usize;
    loop {
        let count = plugin_names().len();
        draw_plugin_menu(stdout, config, selected)?;
        match read_key()? {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(count - 1),
            KeyCode::Char(' ') => toggle_plugin(config, selected),
            KeyCode::Enter | KeyCode::Char('i') => edit_plugin_detail(stdout, config, selected)?,
            _ => {}
        }
    }
}

fn draw_plugin_menu(stdout: &mut io::Stdout, config: &AppConfig, selected: usize) -> Result<()> {
    let (cols, rows) = terminal::size()?;
    let width = cols.saturating_sub(4).max(60);
    let height = rows.saturating_sub(2).max(10);
    let x = 2;
    let y = 1;
    queue!(stdout, Clear(ClearType::All))?;
    draw_box(stdout, x, y, width, height, t(" PLUGINS ", " 插件 "))?;
    queue!(
        stdout,
        MoveTo(x + 2, y + 1),
        Print(t(
            "[Space] enable/disable [Enter] configure [j/k] move [q] back",
            "[Space]启用/禁用 [Enter]配置 [j/k]移动 [q]返回",
        ))
    )?;
    queue!(
        stdout,
        MoveTo(x + 2, y + 3),
        SetAttribute(Attribute::Bold),
        Print(pad(
            &plugin_row(
                t("State", "状态"),
                t("Plugin", "插件"),
                t("Description", "说明"),
                width.saturating_sub(4) as usize,
            ),
            width.saturating_sub(4) as usize,
        )),
        SetAttribute(Attribute::Reset)
    )?;
    let plugins = plugin_names();
    let visible_rows = height.saturating_sub(6) as usize;
    let start = selected.saturating_sub(visible_rows.saturating_sub(1));
    for row in 0..visible_rows {
        let index = start + row;
        if index >= plugins.len() {
            break;
        }
        let (_, name, description) = plugins[index];
        let state = if plugin_enabled(config, index) {
            "[ON]"
        } else {
            "[OFF]"
        };
        let line = plugin_row(state, name, description, width.saturating_sub(4) as usize);
        queue!(stdout, MoveTo(x + 2, y + row as u16 + 4))?;
        if index == selected {
            queue!(
                stdout,
                SetAttribute(Attribute::Reverse),
                Print(pad(&line, width.saturating_sub(4) as usize)),
                SetAttribute(Attribute::Reset)
            )?;
        } else {
            queue!(stdout, Print(pad(&line, width.saturating_sub(4) as usize)))?;
        }
    }
    stdout.flush()?;
    Ok(())
}

fn plugin_row(state: &str, name: &str, description: &str, width: usize) -> String {
    let fixed = pad(state, 8) + &pad(name, 24);
    let remaining = width.saturating_sub(display_width(&fixed)).max(10);
    fixed + &truncate(description, remaining)
}

fn plugin_names() -> [(&'static str, &'static str, &'static str); 15] {
    [
        (
            "web",
            t("Web search", "网络搜索"),
            t(
                "Search APIs and script fallback",
                "搜索 API 与脚本 fallback",
            ),
        ),
        (
            "deep_research",
            t("Deep research", "深度研究"),
            t(
                "Long research tasks with Markdown output",
                "长任务研究并输出 Markdown",
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
    ]
}

pub(super) fn plugin_enabled(config: &AppConfig, index: usize) -> bool {
    match index {
        0 => config.plugins.web.enabled,
        1 => config.plugins.deep_research.enabled,
        2 => config.plugins.vision.enabled,
        3 => config.plugins.image_generation.enabled,
        4 => config.plugins.web_images.enabled,
        5 => config.plugins.print_image.enabled,
        6 => config.plugins.memes.enabled,
        7 => config.plugins.knowledge_base.enabled,
        8 => config.plugins.archlinux.enabled,
        9 => config.plugins.man.enabled,
        10 => config.plugins.memory.enabled,
        11 => config.plugins.package_advisor.enabled,
        12 => config.plugins.linux_game_compatibility.enabled,
        13 => config.plugins.deep_diagnose.enabled,
        14 => config.plugins.diagnostics.enabled,
        _ => false,
    }
}

pub(super) fn toggle_plugin(config: &mut AppConfig, index: usize) {
    let value = !plugin_enabled(config, index);
    match index {
        0 => config.plugins.web.enabled = value,
        1 => config.plugins.deep_research.enabled = value,
        2 => config.plugins.vision.enabled = value,
        3 => config.plugins.image_generation.enabled = value,
        4 => config.plugins.web_images.enabled = value,
        5 => config.plugins.print_image.enabled = value,
        6 => config.plugins.memes.enabled = value,
        7 => config.plugins.knowledge_base.enabled = value,
        8 => config.plugins.archlinux.enabled = value,
        9 => config.plugins.man.enabled = value,
        10 => config.plugins.memory.enabled = value,
        11 => config.plugins.package_advisor.enabled = value,
        12 => config.plugins.linux_game_compatibility.enabled = value,
        13 => config.plugins.deep_diagnose.enabled = value,
        14 => config.plugins.diagnostics.enabled = value,
        _ => {}
    }
}

fn edit_plugin_detail(stdout: &mut io::Stdout, config: &mut AppConfig, index: usize) -> Result<()> {
    let title = format!(" {}: {} ", t("PLUGIN", "插件"), plugin_names()[index].1);
    let mut fields = plugin_fields(config, index);
    if !run_form(stdout, &title, &mut fields)? {
        return Ok(());
    }
    apply_plugin_fields(config, index, &fields)
}
