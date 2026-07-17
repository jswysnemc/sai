use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::Result;
use crossterm::cursor::{Hide, Show};
use crossterm::event::KeyCode;
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use std::io;

use super::agents::edit_agents;
use super::custom_prompts::edit_custom_prompts;
use super::gateways::edit_gateways;
use super::input::read_key;
use super::knowledge::edit_knowledge_base;
use super::plugins::edit_plugins;
use super::providers::{select_active_provider, ProviderBrowser};
use super::settings::edit_settings;
use super::ui::draw_menu;

pub fn run(paths: &SaiPaths) -> Result<()> {
    AppConfig::init_files(paths)?;
    let config = AppConfig::load_or_default(paths)?;
    TerminalSession::start()?.run(paths, config)
}

struct TerminalSession {
    stdout: io::Stdout,
}

impl TerminalSession {
    fn start() -> Result<Self> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, Hide)?;
        Ok(Self { stdout })
    }

    fn run(mut self, paths: &SaiPaths, mut config: AppConfig) -> Result<()> {
        let result = run_main_menu(&mut self.stdout, paths, &mut config);
        execute!(self.stdout, Show, LeaveAlternateScreen)?;
        terminal::disable_raw_mode()?;
        let _ = result?;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = execute!(self.stdout, Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

fn run_main_menu(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    config: &mut AppConfig,
) -> Result<bool> {
    let mut selected = 0usize;
    loop {
        let active = active_label(config);
        let options = [
            format!("{} ({active})", t("Active configuration", "激活配置")),
            t("Providers and models", "供应商和模型").to_string(),
            t("Plugin configuration", "插件配置").to_string(),
            t("Knowledge base", "知识库管理").to_string(),
            t("Gateway channels", "渠道接入").to_string(),
            t("Custom prompts", "自定义提示词").to_string(),
            t("Agent configuration", "Agent 配置").to_string(),
            t("Global settings", "全局参数设置").to_string(),
            t("Save and exit", "保存并退出").to_string(),
        ];
        draw_menu(stdout, " SAI CONFIG ", &options, selected, "")?;

        match read_key()? {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(false),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Enter => match selected {
                0 => select_active_provider(stdout, config)?,
                1 => ProviderBrowser::new(config).run(stdout)?,
                2 => edit_plugins(stdout, config)?,
                3 => edit_knowledge_base(stdout, paths, config)?,
                4 => edit_gateways(stdout, paths, config)?,
                5 => edit_custom_prompts(stdout, paths, config)?,
                6 => edit_agents(stdout, config)?,
                7 => edit_settings(stdout, config)?,
                8 => {
                    config.save(paths)?;
                    return Ok(true);
                }
                _ => {}
            },
            _ => {}
        }
    }
}

fn active_label(config: &AppConfig) -> String {
    config
        .provider(None)
        .map(|provider| format!("{} / {}", provider.display_name, provider.default_model))
        .unwrap_or_else(|_| t("not configured", "未配置").to_string())
}
