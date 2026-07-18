use crate::i18n::text as t;
use anyhow::{bail, Result};

use super::GoalCommand;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ControlSurface {
    Repl,
    Gateway,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ControlCommand {
    Help,
    New {
        title: String,
    },
    /// 恢复/切换会话；`id` 为空时由 REPL/CLI 交互选择
    Resume {
        id: Option<String>,
    },
    Compact,
    Clear {
        all: bool,
    },
    ClearMemory,
    Model {
        selection: Option<usize>,
    },
    Agent {
        selection: Option<usize>,
    },
    Goal(GoalCommand),
}

/// 解析 REPL 或网关控制命令。
///
/// 参数:
/// - `input`: 原始输入文本
/// - `surface`: 命令入口类型
///
/// 返回:
/// - 已识别的控制命令，非控制命令返回空
pub fn parse_control_command(
    input: &str,
    surface: ControlSurface,
) -> Result<Option<ControlCommand>> {
    let Some((name, rest)) = slash_command_parts(input) else {
        return Ok(None);
    };
    let name = name.to_ascii_lowercase();
    if matches_surface_alias(&name, surface, "help", &["帮助"]) {
        return Ok(Some(ControlCommand::Help));
    }
    if matches_surface_alias(&name, surface, "new", &["新建"]) {
        return Ok(Some(ControlCommand::New {
            title: rest.trim().to_string(),
        }));
    }
    if matches_surface_alias(&name, surface, "resume", &["恢复", "续聊"]) {
        let id = rest.trim();
        return Ok(Some(ControlCommand::Resume {
            id: if id.is_empty() {
                None
            } else {
                Some(id.to_string())
            },
        }));
    }
    if matches_surface_alias(&name, surface, "compact", &["压缩"]) {
        // Gateway 兼容旧版 `/压缩 --keep N` 写法；当前实现统一忽略旧参数
        if surface == ControlSurface::Repl && !rest.trim().is_empty() {
            bail!(t(
                "compact command does not accept arguments",
                "compact 命令不接受参数"
            ));
        }
        return Ok(Some(ControlCommand::Compact));
    }
    if matches_surface_alias(&name, surface, "clear", &["清空"]) {
        return parse_clear_command(rest, surface).map(Some);
    }
    if matches_surface_alias(&name, surface, "model", &["模型"]) {
        return Ok(Some(ControlCommand::Model {
            selection: parse_model_args(rest)?,
        }));
    }
    if matches_surface_alias(&name, surface, "agent", &["代理", "智能体"]) {
        return Ok(Some(ControlCommand::Agent {
            selection: parse_model_args(rest)?,
        }));
    }
    if matches_surface_alias(&name, surface, "goal", &["目标"]) {
        return super::goal::parse_goal_command(rest)
            .map(ControlCommand::Goal)
            .map(Some);
    }
    Ok(None)
}

/// 拆分斜杠命令名称和参数。
///
/// 参数:
/// - `input`: 原始输入文本
///
/// 返回:
/// - 命令名和参数文本
fn slash_command_parts(input: &str) -> Option<(&str, &str)> {
    let input = input.trim();
    let input = input
        .strip_prefix('/')
        .or_else(|| input.strip_prefix('／'))?;
    let command_len = input
        .char_indices()
        .find_map(|(index, ch)| ch.is_whitespace().then_some(index))
        .unwrap_or(input.len());
    let command = &input[..command_len];
    let rest = input[command_len..].trim_start();
    Some((command, rest))
}

/// 判断命令是否命中当前入口可用的别名。
///
/// 参数:
/// - `name`: 已归一化的命令名
/// - `surface`: 命令入口类型
/// - `english`: 英文命令名
/// - `gateway_chinese_aliases`: 网关可用中文别名
///
/// 返回:
/// - 命中时返回 true
fn matches_surface_alias(
    name: &str,
    surface: ControlSurface,
    english: &str,
    gateway_chinese_aliases: &[&str],
) -> bool {
    name == english
        || surface == ControlSurface::Gateway
            && gateway_chinese_aliases.iter().any(|alias| name == *alias)
}

/// 解析清空命令参数。
///
/// 参数:
/// - `input`: 参数文本
///
/// 返回:
/// - 是否清空全部记忆
fn parse_clear_command(input: &str, surface: ControlSurface) -> Result<ControlCommand> {
    let parts = input.split_whitespace().collect::<Vec<_>>();
    match parts.as_slice() {
        [] => Ok(ControlCommand::Clear { all: false }),
        [scope] if scope.eq_ignore_ascii_case("all") || *scope == "全部" => {
            Ok(ControlCommand::Clear { all: true })
        }
        [scope]
            if surface == ControlSurface::Repl
                && (scope.eq_ignore_ascii_case("memory") || *scope == "记忆") =>
        {
            Ok(ControlCommand::ClearMemory)
        }
        [scope] => bail!("{}: {scope}", t("unknown clear scope", "未知清空范围")),
        _ => bail!(t("too many clear arguments", "clear 参数过多")),
    }
}

/// 解析模型命令参数。
///
/// 参数:
/// - `input`: 参数文本
///
/// 返回:
/// - 可选模型序号
fn parse_model_args(input: &str) -> Result<Option<usize>> {
    let parts = input.split_whitespace().collect::<Vec<_>>();
    match parts.as_slice() {
        [] => Ok(None),
        [index] => Ok(Some(index.parse::<usize>().map_err(|_| {
            anyhow::anyhow!("{}: {index}", t("invalid model index", "无效模型序号"))
        })?)),
        _ => bail!(t("too many model arguments", "model 参数过多")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_english_and_chinese_gateway_aliases() {
        assert_eq!(
            parse_control_command("/help", ControlSurface::Gateway).unwrap(),
            Some(ControlCommand::Help)
        );
        assert_eq!(
            parse_control_command("/帮助", ControlSurface::Gateway).unwrap(),
            Some(ControlCommand::Help)
        );
        assert_eq!(
            parse_control_command("/压缩 --keep 3", ControlSurface::Gateway).unwrap(),
            Some(ControlCommand::Compact)
        );
    }

    #[test]
    fn parses_clear_and_model_arguments() {
        assert_eq!(
            parse_control_command("/清空 全部", ControlSurface::Gateway).unwrap(),
            Some(ControlCommand::Clear { all: true })
        );
        assert_eq!(
            parse_control_command("/模型 2", ControlSurface::Gateway).unwrap(),
            Some(ControlCommand::Model { selection: Some(2) })
        );
    }

    #[test]
    fn repl_parses_memory_clear_scope() {
        assert_eq!(
            parse_control_command("/clear memory", ControlSurface::Repl).unwrap(),
            Some(ControlCommand::ClearMemory)
        );
    }

    #[test]
    fn parses_agent_command() {
        assert_eq!(
            parse_control_command("/agent", ControlSurface::Repl).unwrap(),
            Some(ControlCommand::Agent { selection: None })
        );
        assert_eq!(
            parse_control_command("/agent 2", ControlSurface::Repl).unwrap(),
            Some(ControlCommand::Agent { selection: Some(2) })
        );
        assert_eq!(
            parse_control_command("/代理 1", ControlSurface::Gateway).unwrap(),
            Some(ControlCommand::Agent { selection: Some(1) })
        );
    }

    #[test]
    fn parses_goal_commands_and_budget() {
        assert_eq!(
            parse_control_command(
                "/goal finish the migration --tokens 20000",
                ControlSurface::Repl
            )
            .unwrap(),
            Some(ControlCommand::Goal(GoalCommand::Set {
                objective: "finish the migration".to_string(),
                token_budget: Some(20_000),
            }))
        );
        assert_eq!(
            parse_control_command("/goal pause", ControlSurface::Repl).unwrap(),
            Some(ControlCommand::Goal(GoalCommand::Pause))
        );
    }

    #[test]
    fn repl_does_not_parse_chinese_slash_commands() {
        assert_eq!(
            parse_control_command("/帮助", ControlSurface::Repl).unwrap(),
            None
        );
        assert_eq!(
            parse_control_command("/压缩", ControlSurface::Repl).unwrap(),
            None
        );
    }

    #[test]
    fn parses_resume_with_optional_id() {
        assert_eq!(
            parse_control_command("/resume", ControlSurface::Repl).unwrap(),
            Some(ControlCommand::Resume { id: None })
        );
        assert_eq!(
            parse_control_command("/resume alpha-1", ControlSurface::Repl).unwrap(),
            Some(ControlCommand::Resume {
                id: Some("alpha-1".to_string())
            })
        );
        assert_eq!(
            parse_control_command("/恢复 work", ControlSurface::Gateway).unwrap(),
            Some(ControlCommand::Resume {
                id: Some("work".to_string())
            })
        );
    }
}
