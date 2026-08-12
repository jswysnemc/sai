//! Skills 管理页：列出全局与人格 Skill，启停、查看详情，并收纳全局开关。
//!
//! 启停通过 `.disabled` 标记文件立即生效；两个全局开关写入内存配置，
//! 由主菜单「保存并退出」落盘。

use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use crate::tools::{list_managed_skills, read_managed_skill, set_managed_skill_enabled};
use anyhow::Result;
use crossterm::event::KeyCode;
use std::io;

use super::input::read_key;
use super::ui::{draw_menu_with_details, message};

/// 全局开关行数量（Skills 总开关 + 允许执行命令）。
const TOGGLE_ROWS: usize = 2;

/// 管理 Skills 列表与全局开关。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `paths`: Sai 路径（扫描 Skills 目录）
/// - `config`: 待更新应用配置
///
/// 返回:
/// - 管理流程是否成功
pub(crate) fn edit_skills(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    config: &mut AppConfig,
) -> Result<()> {
    let mut selected = 0usize;
    let mut status = String::new();
    loop {
        let skills = list_managed_skills(config, paths).unwrap_or_default();
        let enabled_count = skills.iter().filter(|skill| skill.enabled).count();

        let mut options = vec![
            format!(
                "{} {}",
                state_dot(config.skills.enabled),
                t("Skills enabled", "Skills 总开关")
            ),
            format!(
                "{} {}",
                state_dot(config.skills.allow_command_execution),
                t("Allow command execution", "允许 Skill 执行命令")
            ),
        ];
        let mut details = vec![
            t(
                "Master switch: when off, no skill is offered to the model. Saved to config in memory; persist via Save & Exit.",
                "总开关：关闭后不向模型提供任何 Skill。写入内存配置，经主菜单「保存并退出」落盘。",
            )
            .to_string(),
            t(
                "Allow skills to run the commands they declare. Keep off to restrict skills to instructions only.",
                "允许 Skill 执行其声明的命令。关闭则 Skill 仅提供指令说明。",
            )
            .to_string(),
        ];
        if skills.is_empty() {
            options.push(format!(
                "  ({})",
                t("no skills installed", "尚未安装 Skill")
            ));
            details.push(
                t(
                    "Put skill directories with SKILL.md into the skills directory, or install via `sai skills`.",
                    "将包含 SKILL.md 的目录放入 skills 目录，或通过 `sai skills` 命令安装。",
                )
                .to_string(),
            );
        } else {
            for skill in &skills {
                options.push(format!(
                    "{} {}  [{}]",
                    state_dot(skill.enabled),
                    skill.name,
                    skill.scope
                ));
                details.push(format!(
                    "{}\n\n{}: {}\n{}: {}",
                    skill.description,
                    t("Scope", "作用域"),
                    skill.scope,
                    t("Path", "路径"),
                    skill.path,
                ));
            }
        }
        selected = selected.min(options.len().saturating_sub(1));

        let subtitle = format!(
            "{}: {enabled_count}/{} · {}",
            t("Enabled", "已启用"),
            skills.len(),
            if config.skills.enabled {
                t("skills on", "Skills 开启")
            } else {
                t("skills off", "Skills 关闭")
            }
        );
        let help = if status.is_empty() {
            super::theme::help_line(&[
                ("Space", t("toggle", "启停")),
                ("Enter", t("view / toggle", "查看/切换")),
                ("q", t("back", "返回")),
            ])
        } else {
            status.clone()
        };
        draw_menu_with_details(
            stdout, " SKILLS ", &options, &details, selected, &help, &subtitle,
        )?;

        let skill_index = selected.checked_sub(TOGGLE_ROWS);
        match read_key()? {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => {
                selected = selected.saturating_sub(1);
                status.clear();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                selected = (selected + 1).min(options.len().saturating_sub(1));
                status.clear();
            }
            KeyCode::Char(' ') | KeyCode::Enter if selected == 0 => {
                config.skills.enabled = !config.skills.enabled;
            }
            KeyCode::Char(' ') | KeyCode::Enter if selected == 1 => {
                config.skills.allow_command_execution = !config.skills.allow_command_execution;
            }
            KeyCode::Char(' ') => {
                if let Some(skill) = skill_index.and_then(|index| skills.get(index)) {
                    status =
                        match set_managed_skill_enabled(&skill.id, !skill.enabled, config, paths) {
                            Ok(()) => format!(
                                "{}: {}",
                                if skill.enabled {
                                    t("disabled", "已禁用")
                                } else {
                                    t("enabled", "已启用")
                                },
                                skill.name
                            ),
                            Err(err) => err.to_string(),
                        };
                }
            }
            KeyCode::Enter => {
                if let Some(skill) = skill_index.and_then(|index| skills.get(index)) {
                    view_skill(stdout, config, paths, &skill.id)?;
                }
            }
            _ => {}
        }
    }
}

/// 用任意键返回的信息页展示 SKILL.md 内容摘要。
fn view_skill(
    stdout: &mut io::Stdout,
    config: &AppConfig,
    paths: &SaiPaths,
    id: &str,
) -> Result<()> {
    match read_managed_skill(id, config, paths) {
        Ok((skill, content)) => {
            // message 页按屏高截断，只取头部内容避免构造超长字符串
            let preview: String = content.chars().take(4000).collect();
            message(
                stdout,
                &format!(
                    "{} [{}]\n{}\n\n{preview}",
                    skill.name, skill.scope, skill.path
                ),
            )
        }
        Err(err) => message(
            stdout,
            &format!("{}: {err}", t("Failed to read skill", "读取 Skill 失败")),
        ),
    }
}

/// 启用状态圆点。
fn state_dot(enabled: bool) -> &'static str {
    if enabled {
        "●"
    } else {
        "○"
    }
}
