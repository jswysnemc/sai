use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::Result;
use crossterm::cursor::{Hide, Show};
use crossterm::event::KeyCode;
use crossterm::execute;
use crossterm::terminal;
use std::io;

use super::agents::edit_agents;
use super::dashboard::{self, MainAction};
use super::gateways::edit_gateways;
use super::input::{read_key, read_key_event, Interrupted};
use super::knowledge::edit_knowledge_base;
use super::plugins::edit_cli_tools;
use super::providers::{select_active_provider, ProviderBrowser};
use super::settings::{
    edit_display_and_context_menu, edit_permissions_and_sandbox_menu, edit_settings,
};
use super::skills::edit_skills;
use super::ui::{confirm_unsaved_exit, draw_menu_with_details, message, UnsavedExitChoice};
use crossterm::event::KeyModifiers;

pub fn run(paths: &SaiPaths) -> Result<()> {
    AppConfig::init_files(paths)?;
    let config = AppConfig::load_or_default(paths)?;
    TerminalSession::start()?.run(paths, config)
}

/// 【首次配置】【终端会话】在统一终端守卫内运行首次供应商表单，退出时恢复屏幕和输入法。
/// @param paths 为应用路径；config 为当前配置
/// @returns 用户完成配置时为 true；取消时为 false
pub(super) fn run_provider_setup(paths: &SaiPaths, config: &AppConfig) -> Result<bool> {
    let mut terminal = TerminalSession::start()?;
    match super::onboarding::run(&mut terminal.stdout, paths, config) {
        Err(error) if error.downcast_ref::<Interrupted>().is_some() => Ok(false),
        result => result,
    }
}

struct TerminalSession {
    stdout: io::Stdout,
    _ime: super::ime::ImeGuard,
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
        crate::render::terminal_palette::initialize_terminal_palette();
        // 1. 先开启 raw mode
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        // 2. 进入备用屏失败时 Self 未构造、Drop 不会执行，须手工回滚 raw mode
        if let Err(err) = crate::cli::alternate_screen::enter_alternate_screen(&mut stdout)
            .and_then(|_| execute!(stdout, Hide))
        {
            let _ = terminal::disable_raw_mode();
            return Err(err.into());
        }
        // 输入法开着时 `/` 和方向键到不了界面。导航期间关掉，文本框里再打开。
        Ok(Self {
            stdout,
            _ime: super::ime::ImeGuard::suspend(),
        })
    }

    fn run(mut self, paths: &SaiPaths, mut config: AppConfig) -> Result<()> {
        let result = run_main_menu(&mut self.stdout, paths, &mut config);
        execute!(self.stdout, Show)?;
        crate::cli::alternate_screen::leave_alternate_screen(&mut self.stdout)?;
        terminal::disable_raw_mode()?;
        match result {
            Ok(_) => Ok(()),
            // Ctrl+C 是用户主动放弃，不是故障：安静退出，不打印错误栈
            Err(err) if err.downcast_ref::<Interrupted>().is_some() => Ok(()),
            Err(err) => Err(err),
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = execute!(self.stdout, Show);
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

fn run_main_menu(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    config: &mut AppConfig,
) -> Result<bool> {
    let mut selected = 0usize;
    // 1. 【配置】【基线快照】进入时的序列化快照；直接保存后同步刷新基线
    let mut baseline = serde_json::to_string(&*config).ok();
    let mut notice: Option<String> = None;
    loop {
        let dirty = has_unsaved_changes(config, baseline.as_deref());
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
        draw_menu_with_details(
            stdout,
            if dirty { "SAI CONFIG *" } else { "SAI CONFIG" },
            &options,
            &details,
            selected,
            &status,
            &subtitle,
        )?;

        let event = read_key_event()?;
        if event.code == KeyCode::Char('s') && event.modifiers.contains(KeyModifiers::CONTROL) {
            if save_now(stdout, paths, config)? {
                baseline = serde_json::to_string(&*config).ok();
                notice = Some(t("Saved", "已保存").to_string());
            }
            continue;
        }
        match event.code {
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
            // 2. 【配置】【单键保存】主界面按 s / w 直接写盘并驻留，不打断继续调参
            KeyCode::Char('s' | 'w') => {
                if !dirty {
                    notice = Some(t("No changes", "无新改动").to_string());
                } else if save_now(stdout, paths, config)? {
                    baseline = serde_json::to_string(&*config).ok();
                    notice = Some(t("Saved", "已保存").to_string());
                }
            }
            // 3. 【配置】【单键快切】对高频项直接在主菜单按 Space / 左右键切换生效
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
                    MainAction::Advanced => run_advanced_menu(stdout, paths, config)?,
                    MainAction::ExitOrSave => {
                        // 4. 【配置】【零二次确认】明确选择「保存并退出」时直接落盘退出
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
///
/// 参数:
/// - `stdout`: 终端输出
/// - `paths`: Sai 路径
/// - `config`: 当前内存配置
///
/// 返回:
/// - 校验与写盘均成功时返回 true
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
///
/// 参数:
/// - `stdout`: 终端输出
/// - `paths`: Sai 路径
/// - `config`: 当前内存配置
/// - `dirty`: 是否相对进入时发生了改动
///
/// 返回:
/// - `Some(true)` 已保存后退出；`Some(false)` 放弃或无需保存后退出；`None` 继续编辑
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
///
/// 参数:
/// - `config`: 当前内存配置
/// - `baseline`: 进入时的序列化快照；取不到时按「有改动」处理
///
/// 返回:
/// - 需要落盘时返回 true
fn has_unsaved_changes(config: &AppConfig, baseline: Option<&str>) -> bool {
    let Some(baseline) = baseline else {
        // 快照不可用时宁可多存一次，也不能把用户的编辑丢掉
        return true;
    };
    serde_json::to_string(config)
        .map(|current| current != baseline)
        .unwrap_or(true)
}

/// 高级设置二级菜单：知识库、渠道接入、全局参数与 Jev 等低频配置。///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `paths`: Sai 路径
/// - `config`: 待更新应用配置
///
/// 返回:
/// - 菜单退出结果
fn run_advanced_menu(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    config: &mut AppConfig,
) -> Result<()> {
    let mut selected = 0usize;
    loop {
        let labels = [
            t("Knowledge base", "知识库管理"),
            t("Gateway channels", "渠道接入"),
            t("Global parameters", "全局参数"),
            "Jev",
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
        draw_menu_with_details(
            stdout,
            t(" ADVANCED ", " 高级设置 "),
            &options,
            &details,
            selected,
            &super::theme::help_line(&[
                ("↑↓", t("move", "移动")),
                ("Enter", t("open", "打开")),
                ("q", t("back", "返回")),
            ]),
            "",
        )?;
        match read_key()? {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Char(digit @ '1'..='4') => {
                selected = digit as usize - '1' as usize;
            }
            KeyCode::Enter => match selected {
                0 => edit_knowledge_base(stdout, paths, config)?,
                1 => edit_gateways(stdout, paths, config)?,
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

    /// 【配置】【自动保存】验证新增模型会被判定为待落盘改动。
    ///
    /// 判定失败等于按 q 退出后模型丢失，/model 里找不到刚添加的条目。
    #[test]
    fn added_model_counts_as_an_unsaved_change() {
        let mut config = AppConfig::default();
        let baseline = serde_json::to_string(&config).unwrap();
        assert!(!has_unsaved_changes(&config, Some(&baseline)));

        config.providers[0].models.push("new-model".to_string());

        assert!(has_unsaved_changes(&config, Some(&baseline)));
    }

    /// 【配置】【自动保存】验证快照缺失时按有改动处理。
    #[test]
    fn missing_baseline_forces_a_save() {
        let config = AppConfig::default();

        assert!(has_unsaved_changes(&config, None));
    }
}
