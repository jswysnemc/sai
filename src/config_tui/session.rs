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
use super::gateways::edit_gateways;
use super::input::read_key;
use super::knowledge::edit_knowledge_base;
use super::plugins::{edit_cli_tools, edit_web_search};
use super::providers::{select_active_provider, ProviderBrowser};
use super::settings::edit_settings;
use super::ui::draw_menu_with_details;

pub fn run(paths: &SaiPaths) -> Result<()> {
    AppConfig::init_files(paths)?;
    let config = AppConfig::load_or_default(paths)?;
    TerminalSession::start()?.run(paths, config)
}

struct TerminalSession {
    stdout: io::Stdout,
}

impl TerminalSession {
    /// 启动配置界面终端会话。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 初始化完成的终端会话；进入备用屏失败时回滚 raw mode 后返回错误
    fn start() -> Result<Self> {
        // 1. 先开启 raw mode
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        // 2. 进入备用屏失败时 Self 未构造、Drop 不会执行，须手工回滚 raw mode
        if let Err(err) = execute!(stdout, EnterAlternateScreen, Hide) {
            let _ = terminal::disable_raw_mode();
            return Err(err.into());
        }
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
            t("Active configuration", "激活配置").to_string(),
            t("Providers and models", "供应商和模型").to_string(),
            t("Web search", "Web 搜索").to_string(),
            t("CLI assistant tools", "CLI 助手工具").to_string(),
            t("Knowledge base", "知识库管理").to_string(),
            t("Gateway channels", "渠道接入").to_string(),
            t("Agent configuration", "Agent 配置").to_string(),
            t("Global settings", "全局参数设置").to_string(),
            t("Save and exit", "保存并退出").to_string(),
        ];
        let details = [
            format!(
                "{}\n\n{}: {active}",
                t(
                    "Pick which provider/model is active for new chats.",
                    "选择新对话默认使用的供应商与模型。",
                ),
                t("Current", "当前")
            ),
            t(
                "Browse providers, organizations and models. Activate, add, delete or refresh the catalog.",
                "浏览供应商、组织与模型；可激活、添加、删除或刷新目录。",
            )
            .to_string(),
            t(
                "Configure web search backends and related API credentials.",
                "配置 Web 搜索后端与相关 API 凭证。",
            )
            .to_string(),
            t(
                "Enable or configure CLI assistant tools available to the agent.",
                "启用或配置智能体可用的 CLI 助手工具。",
            )
            .to_string(),
            t(
                "Manage knowledge bases used for retrieval during conversations.",
                "管理对话检索所用的知识库。",
            )
            .to_string(),
            t(
                "Connect messaging gateways such as Telegram or other channels.",
                "接入 Telegram 等消息渠道。",
            )
            .to_string(),
            t(
                "Create and edit agent profiles, tools and system prompts.",
                "创建与编辑 Agent 配置、工具与系统提示。",
            )
            .to_string(),
            t(
                "Permissions, context limits, display preferences and tool defaults.",
                "权限模式、上下文上限、显示偏好与工具默认值。",
            )
            .to_string(),
            t(
                "Write all pending changes to disk and leave the configuration UI.",
                "将未保存的更改写入磁盘并退出配置界面。",
            )
            .to_string(),
        ];
        let subtitle = format!("{}: {active}", t("Active", "当前激活"));
        draw_menu_with_details(
            stdout,
            " SAI CONFIG ",
            &options,
            &details,
            selected,
            "",
            &subtitle,
        )?;

        match read_key()? {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(false),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Enter => match selected {
                0 => select_active_provider(stdout, config)?,
                1 => ProviderBrowser::new(config).run(stdout)?,
                2 => edit_web_search(stdout, config)?,
                3 => edit_cli_tools(stdout, config)?,
                4 => edit_knowledge_base(stdout, paths, config)?,
                5 => edit_gateways(stdout, paths, config)?,
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
