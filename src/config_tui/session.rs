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
use super::input::{read_key, Interrupted};
use super::knowledge::edit_knowledge_base;
use super::plugins::edit_cli_tools;
use super::providers::{select_active_provider, ProviderBrowser};
use super::settings::edit_settings;
use super::skills::edit_skills;
use super::ui::{confirm_unsaved_exit, draw_menu_with_details, message, UnsavedExitChoice};

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
    // 进入时的快照：用来标已修改状态，退出前提示保存或放弃
    let baseline = serde_json::to_string(&*config).ok();
    loop {
        let active = active_label(config);
        let dirty = has_unsaved_changes(config, baseline.as_deref());
        // 高频操作前置，低频配置收进「高级设置」；编号既是视觉锚点也是直达键
        let labels = [
            t("Active configuration", "激活配置"),
            t("Providers and models", "供应商和模型"),
            t("Agent configuration", "Agent 配置"),
            t("Tools", "工具"),
            "Skills",
            t("Advanced settings", "高级设置"),
            if dirty {
                t("Save and exit", "保存并退出")
            } else {
                t("Exit", "退出")
            },
        ];
        let options = labels
            .iter()
            .enumerate()
            .map(|(index, label)| format!("{}  {label}", index + 1))
            .collect::<Vec<_>>();
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
                "Create and edit agent profiles: model, prompt, tool capabilities and skills.",
                "创建与编辑 Agent 档案：模型、提示词、工具能力与 Skills。",
            )
            .to_string(),
            t(
                "Toggle and configure assistant tools, including web search.",
                "启用与配置助手工具，含 Web 搜索。",
            )
            .to_string(),
            t(
                "Enable or disable installed skills and global skill switches.",
                "启停已安装的 Skill 与全局 Skill 开关。",
            )
            .to_string(),
            t(
                "Less frequent settings: knowledge base, gateway channels and global parameters.",
                "低频配置：知识库、渠道接入与全局参数。",
            )
            .to_string(),
            if dirty {
                t(
                    "Configuration has unsaved edits. Confirm whether to write them before leaving.",
                    "配置有未保存的修改。退出前确认是否写入磁盘。",
                )
                .to_string()
            } else {
                t(
                    "No pending edits. Leave the configuration UI.",
                    "没有未保存的修改。离开配置界面。",
                )
                .to_string()
            },
        ];
        let subtitle = if dirty {
            format!(
                "{}: {active}  ·  {}",
                t("Active", "当前激活"),
                t("Unsaved changes", "已修改")
            )
        } else {
            format!("{}: {active}", t("Active", "当前激活"))
        };
        let status = super::theme::help_line(&[
            ("↑↓", t("move", "移动")),
            ("1-7", t("jump", "跳转")),
            ("Enter", t("open", "打开")),
            ("q", t("quit", "退出")),
        ]);
        draw_menu_with_details(
            stdout,
            if dirty {
                "SAI CONFIG *"
            } else {
                "SAI CONFIG"
            },
            &options,
            &details,
            selected,
            &status,
            &subtitle,
        )?;

        match read_key()? {
            KeyCode::Char('q') | KeyCode::Esc => {
                if let Some(saved) = confirm_and_leave(stdout, paths, config, dirty)? {
                    return Ok(saved);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Char(digit @ '1'..='9') => {
                let target = digit as usize - '1' as usize;
                if target < options.len() {
                    selected = target;
                }
            }
            KeyCode::Enter => match selected {
                0 => select_active_provider(stdout, config)?,
                1 => ProviderBrowser::new(config).run(stdout)?,
                2 => edit_agents(stdout, paths, config)?,
                3 => edit_cli_tools(stdout, config)?,
                4 => edit_skills(stdout, paths, config)?,
                5 => run_advanced_menu(stdout, paths, config)?,
                6 => {
                    if dirty {
                        if let Some(saved) = confirm_and_leave(stdout, paths, config, true)? {
                            return Ok(saved);
                        }
                    } else {
                        return Ok(false);
                    }
                }
                _ => {}
            },
            _ => {}
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
        UnsavedExitChoice::Save => {
            // 必须在落盘前校验：validate 只在加载时跑，写出非法配置会让下一次
            // sai（包括 sai config 本身）启动即失败，用户被锁在唯一能修复它的界面外
            if let Err(error) = config.validate() {
                message(
                    stdout,
                    &format!(
                        "{}\n\n{error}",
                        t(
                            "Configuration is invalid, not saved. Fix it before leaving.",
                            "配置校验未通过，未保存。请先修正后再退出。"
                        )
                    ),
                )?;
                return Ok(None);
            }
            match config.save(paths) {
                Ok(()) => Ok(Some(true)),
                Err(error) => {
                    message(
                        stdout,
                        &format!(
                            "{}: {error}",
                            t("Failed to save configuration", "保存配置失败")
                        ),
                    )?;
                    Ok(None)
                }
            }
        }
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

/// 高级设置二级菜单：知识库、渠道接入与全局参数等低频配置。///
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
            KeyCode::Char(digit @ '1'..='3') => {
                selected = digit as usize - '1' as usize;
            }
            KeyCode::Enter => match selected {
                0 => edit_knowledge_base(stdout, paths, config)?,
                1 => edit_gateways(stdout, paths, config)?,
                2 => edit_settings(stdout, config)?,
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
