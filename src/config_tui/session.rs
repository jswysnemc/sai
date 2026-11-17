//! 配置中心终端会话管理、主控制台导航、未保存更改保护与双语顶栏。

use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::Result;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste, KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal;
use std::io;

use super::agents::edit_agents;
use super::dashboard::{self, MainAction};
use super::gateways::edit_gateways;
use super::input::{Interrupted, UnsavedState};
use super::knowledge::edit_knowledge_base;
use super::mouse::{ListMouseState, MenuInput};
use super::plugins::edit_cli_tools;
use super::providers::{select_active_provider, ProviderBrowser};
use super::settings::{
    edit_display_and_context_menu, edit_permissions_and_sandbox_menu, edit_settings,
};
use super::skills::edit_skills;
use super::ui::{confirm_unsaved_exit, draw_menu_interactive, message, UnsavedExitChoice};

/// 【配置中心】【命令行入口】初始化配置文件并进入全屏配置终端会话。
///
/// 参数:
/// - `paths`: Sai 存储路径集合
///
/// 返回:
/// - 会话执行结果
pub fn run(paths: &SaiPaths) -> Result<()> {
    AppConfig::init_files(paths)?;
    let config = AppConfig::load_or_default(paths)?;
    TerminalSession::start()?.run(paths, config)
}

/// 【首次配置】【终端会话】在统一终端守卫内运行首次供应商表单，退出时恢复屏幕和输入法。
///
/// 参数:
/// - `paths`: 应用路径
/// - `config`: 当前配置
///
/// 返回:
/// - 用户完成配置时为 true；取消时为 false
pub(super) fn run_provider_setup(paths: &SaiPaths, config: &AppConfig) -> Result<bool> {
    let mut terminal = TerminalSession::start()?;
    match super::onboarding::run(&mut terminal.stdout, paths, config) {
        Err(error) if error.downcast_ref::<Interrupted>().is_some() => Ok(false),
        result => result,
    }
}

struct TerminalSession {
    stdout: io::Stdout,
    _mouse: Option<crate::cli::mouse_capture::MouseCapture>,
    _ime: super::ime::ImeGuard,
}

impl TerminalSession {
    /// 【配置中心】【启动终端会话】探测背景色并开启原始模式、备用屏、括号粘贴与鼠标捕获。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 初始化完成的终端会话守卫
    fn start() -> Result<Self> {
        crate::render::terminal_palette::initialize_terminal_palette();
        // 1. 【配置中心】【原始模式】先启用终端 raw mode
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        // 2. 【配置中心】【备用屏与粘贴】进入备用屏并启用括号粘贴以支持 Event::Paste
        if let Err(err) = crate::cli::alternate_screen::enter_alternate_screen(&mut stdout)
            .and_then(|_| execute!(stdout, EnableBracketedPaste, Hide))
        {
            let _ = terminal::disable_raw_mode();
            return Err(err.into());
        }
        // 3. 【配置中心】【鼠标捕获】开启共享鼠标捕获；失败时完整回滚全部终端状态
        let mouse = match crate::cli::mouse_capture::MouseCapture::acquire() {
            Ok(mouse) => mouse,
            Err(err) => {
                let _ = execute!(stdout, DisableBracketedPaste, Show);
                let _ = crate::cli::alternate_screen::leave_alternate_screen(&mut stdout);
                let _ = terminal::disable_raw_mode();
                return Err(err.into());
            }
        };
        Ok(Self {
            stdout,
            _mouse: Some(mouse),
            _ime: super::ime::ImeGuard::suspend(),
        })
    }

    fn run(mut self, paths: &SaiPaths, mut config: AppConfig) -> Result<()> {
        let result = run_main_menu(&mut self.stdout, paths, &mut config);
        self._mouse.take();
        execute!(self.stdout, DisableBracketedPaste, Show)?;
        crate::cli::alternate_screen::leave_alternate_screen(&mut self.stdout)?;
        terminal::disable_raw_mode()?;
        match result {
            Ok(_) => Ok(()),
            Err(err) if err.downcast_ref::<Interrupted>().is_some() => Ok(()),
            Err(err) => Err(err),
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        self._mouse.take();
        let _ = execute!(self.stdout, DisableBracketedPaste, Show);
        let _ = crate::cli::alternate_screen::leave_alternate_screen(&mut self.stdout);
        let _ = execute!(
            self.stdout,
            crossterm::style::Print(crate::render::readable_terminal_text(
                &("\u{1b}[?2026l").to_string()
            ))
        );
        let _ = terminal::disable_raw_mode();
    }
}

/// 【配置中心】【主菜单循环】展示扁平导航面板并维护未保存基线快照与中断保护。
fn run_main_menu(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    config: &mut AppConfig,
) -> Result<bool> {
    let mut selected = 0usize;
    let mut mouse = ListMouseState::default();
    // 1. 【配置中心】【基线与中断保护】记录初始序列化快照并同步至输入层，防止未保存时 Ctrl+C 静默丢弃修改
    let mut baseline = serde_json::to_string(&*config).ok();
    let mut unsaved_guard = UnsavedState::default();
    let mut notice: Option<String> = None;
    loop {
        let dirty = has_unsaved_changes(config, baseline.as_deref());
        unsaved_guard.set(dirty);

        let items = dashboard::menu_items(config, dirty);
        let options = items
            .iter()
            .map(|item| item.label.clone())
            .collect::<Vec<_>>();
        let details = items
            .iter()
            .map(|item| item.detail.clone())
            .collect::<Vec<_>>();
        selected = selected.min(items.len().saturating_sub(1));
        let subtitle = dashboard::header_subtitle(config, dirty, notice.as_deref());
        let status = super::theme::help_line(&[
            ("↑↓", t("move", "移动")),
            ("Space", t("cycle", "快切")),
            ("Enter", t("open", "打开")),
            ("s", t("save", "保存")),
            ("q", t("quit", "退出")),
        ]);
        let title = if dirty {
            " SAI CONFIG * "
        } else {
            " SAI CONFIG "
        };
        let hitbox = draw_menu_interactive(
            stdout, title, &options, &details, selected, &mouse, &status, &subtitle,
        )?;

        let code = match hitbox.read(&mut mouse, selected, false)? {
            MenuInput::Redraw | MenuInput::Paste(_) => continue,
            MenuInput::Select(target) => {
                selected = target;
                notice = None;
                continue;
            }
            MenuInput::Activate(target) => {
                selected = target;
                KeyCode::Enter
            }
            MenuInput::RightClick(target) => {
                selected = target;
                KeyCode::Char(' ')
            }
            MenuInput::Action(code) => code,
            MenuInput::Key(event) => {
                if event.code == KeyCode::Char('s')
                    && event.modifiers.contains(KeyModifiers::CONTROL)
                {
                    if save_now(stdout, paths, config)? {
                        baseline = serde_json::to_string(&*config).ok();
                        notice = Some(t("Saved", "已保存").to_string());
                    }
                    continue;
                }
                event.code
            }
        };
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                if let Some(saved) = confirm_and_leave(stdout, paths, config, dirty)? {
                    return Ok(saved);
                }
                notice = None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                selected = selected.saturating_sub(1);
                notice = None;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                selected = (selected + 1).min(options.len() - 1);
                notice = None;
            }
            KeyCode::Char(digit @ '1'..='9') => {
                let target = digit as usize - '1' as usize;
                if target < options.len() {
                    selected = target;
                    notice = None;
                }
            }
            KeyCode::Char('0') => {
                selected = options.len().saturating_sub(1);
                notice = None;
            }
            // 2. 【配置中心】【单键写盘】主界面按 s / w 直接写盘并刷新基线，驻留继续调参
            KeyCode::Char('s' | 'w') => {
                if !dirty {
                    notice = Some(t("No changes", "无新改动").to_string());
                } else if save_now(stdout, paths, config)? {
                    baseline = serde_json::to_string(&*config).ok();
                    notice = Some(t("Saved", "已保存").to_string());
                }
            }
            // 3. 【配置中心】【行内快切】对高频项直接在主菜单按 Space / 左右键切换生效
            KeyCode::Char(' ') | KeyCode::Right | KeyCode::Char('l') => {
                if let Some(feedback) = dashboard::quick_cycle(config, items[selected].action, true)
                {
                    notice = Some(feedback);
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if let Some(feedback) =
                    dashboard::quick_cycle(config, items[selected].action, false)
                {
                    notice = Some(feedback);
                }
            }
            KeyCode::Enter => {
                notice = None;
                match items[selected].action {
                    MainAction::ActiveModel => select_active_provider(stdout, config)?,
                    MainAction::Providers => ProviderBrowser::new(config).run(stdout)?,
                    MainAction::Agents => edit_agents(stdout, paths, config)?,
                    MainAction::PermissionsAndSandbox => {
                        edit_permissions_and_sandbox_menu(stdout, config)?
                    }
                    MainAction::DisplayAndContext => edit_display_and_context_menu(stdout, config)?,
                    MainAction::Tools => edit_cli_tools(stdout, config)?,
                    MainAction::Skills => edit_skills(stdout, paths, config)?,
                    MainAction::ModelEndpoints => super::model_endpoints::menu(stdout, config)?,
                    MainAction::Advanced => {
                        run_advanced_menu(stdout, paths, config, &mut baseline)?
                    }
                    MainAction::ExitOrSave => {
                        if dirty {
                            if save_now(stdout, paths, config)? {
                                return Ok(true);
                            }
                        } else {
                            return Ok(false);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// 校验并立即把当前配置写入磁盘。
fn save_now(stdout: &mut io::Stdout, paths: &SaiPaths, config: &AppConfig) -> Result<bool> {
    if let Err(error) = config.validate() {
        message(
            stdout,
            &format!(
                "{}\n\n{error}",
                t(
                    "Configuration is invalid, not saved. Fix it before leaving.",
                    "配置校验未通过，未保存。请先修正后再保存。",
                )
            ),
        )?;
        return Ok(false);
    }
    match config.save(paths) {
        Ok(()) => Ok(true),
        Err(error) => {
            message(
                stdout,
                &format!(
                    "{}: {error}",
                    t("Failed to save configuration", "保存配置失败")
                ),
            )?;
            Ok(false)
        }
    }
}

/// 退出前处理未保存更改。
fn confirm_and_leave(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    config: &AppConfig,
    dirty: bool,
) -> Result<Option<bool>> {
    if !dirty {
        return Ok(Some(false));
    }
    match confirm_unsaved_exit(stdout)? {
        UnsavedExitChoice::Save => Ok(save_now(stdout, paths, config)?.then_some(true)),
        UnsavedExitChoice::Discard => Ok(Some(false)),
        UnsavedExitChoice::Cancel => Ok(None),
    }
}

/// 判断配置相对进入配置界面时是否发生了改动。
pub(super) fn has_unsaved_changes(config: &AppConfig, baseline: Option<&str>) -> bool {
    let Some(baseline) = baseline else {
        return true;
    };
    serde_json::to_string(config)
        .map(|current| current != baseline)
        .unwrap_or(true)
}

/// 【配置中心】【高级设置菜单】聚合知识库、渠道接入、全局参数与 Jev 设置，并透传基线快照。
fn run_advanced_menu(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    config: &mut AppConfig,
    baseline: &mut Option<String>,
) -> Result<()> {
    let mut selected = 0usize;
    let mut mouse = ListMouseState::default();
    loop {
        let labels = [
            t("Knowledge base", "知识库管理"),
            t("Gateway channels", "渠道接入"),
            t("Global parameters", "全局参数"),
            t("Jev settings", "Jev 设置"),
        ];
        let options = labels
            .iter()
            .enumerate()
            .map(|(index, label)| format!("{}  {label}", index + 1))
            .collect::<Vec<_>>();
        let details = [
            t(
                "Manage knowledge bases used for retrieval during conversations.",
                "管理对话检索所用的知识库。",
            )
            .to_string(),
            t(
                "Connect messaging gateways such as QQ or Weixin channels.",
                "接入 QQ、微信等消息渠道。",
            )
            .to_string(),
            t(
                "Permissions, context limits, tool behavior and display preferences.",
                "权限模式、上下文上限、工具行为与显示偏好。",
            )
            .to_string(),
            t(
                "Built-in TypeSafe Jev: tool and skill routing, permission audit and connection test.",
                "内置 TypeSafe Jev：工具与 Skills 暴露决策、权限自动审核与连接测试。",
            )
            .to_string(),
        ];
        let hitbox = draw_menu_interactive(
            stdout,
            t(" ADVANCED ", " 高级设置 "),
            &options,
            &details,
            selected,
            &mouse,
            &super::theme::help_line(&[
                ("↑↓", t("move", "移动")),
                ("Enter", t("open", "打开")),
                ("q", t("back", "返回")),
            ]),
            "",
        )?;
        let code = match hitbox.read(&mut mouse, selected, false)? {
            MenuInput::Redraw | MenuInput::Paste(_) => continue,
            MenuInput::Select(index) | MenuInput::RightClick(index) => {
                selected = index;
                continue;
            }
            MenuInput::Activate(index) => {
                selected = index;
                KeyCode::Enter
            }
            MenuInput::Key(key) => key.code,
            MenuInput::Action(code) => code,
        };
        match code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Char(digit @ '1'..='4') => {
                selected = digit as usize - '1' as usize;
            }
            KeyCode::Enter => match selected {
                0 => edit_knowledge_base(stdout, paths, config)?,
                1 => edit_gateways(stdout, paths, config, baseline)?,
                2 => edit_settings(stdout, config)?,
                3 => super::jev::edit_jev(stdout, config)?,
                _ => {}
            },
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn added_model_counts_as_an_unsaved_change() {
        let mut config = AppConfig::default();
        let baseline = serde_json::to_string(&config).unwrap();
        assert!(!has_unsaved_changes(&config, Some(&baseline)));

        config.providers[0].models.push("new-model".to_string());

        assert!(has_unsaved_changes(&config, Some(&baseline)));
    }

    #[test]
    fn missing_baseline_forces_a_save() {
        let config = AppConfig::default();

        assert!(has_unsaved_changes(&config, None));
    }
}
