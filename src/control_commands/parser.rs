use crate::i18n::text as t;
use anyhow::{bail, Result};

use super::GoalCommand;

/// `/context` 对本会话压缩策略的改写。
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ContextPolicyUpdate {
    /// 清除会话覆盖，回到全局默认
    Reset,
    /// 写入本会话比例（50–99），可选同时改预留
    Set {
        ratio_percent: u32,
        reserve: Option<usize>,
    },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ControlSurface {
    Repl,
    Gateway,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ControlCommand {
    Help,
    /// 查看或改写当前会话的上下文压缩策略
    Context {
        update: Option<ContextPolicyUpdate>,
    },
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
    /// 打开会话树；`turn_id` 为空时交互选择
    Tree {
        turn_id: Option<String>,
    },
    /// 重命名当前会话
    Rename {
        title: String,
    },
    Goal(GoalCommand),
    /// 列出当前会话的后台子智能体
    Subagents,
    /// 给子智能体留言；`target` 为空时投递给当前查看或唯一存活的子智能体
    SubagentMessage {
        target: Option<String>,
        message: String,
    },
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
    if matches_surface_alias(&name, surface, "context", &["上下文"]) {
        return Ok(Some(ControlCommand::Context {
            update: parse_context_policy_update(rest)?,
        }));
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
    if matches_surface_alias(&name, surface, "rename", &["重命名"]) {
        let title = rest.trim();
        if title.is_empty() {
            bail!(t(
                "rename command requires a title",
                "rename 命令需要提供标题"
            ));
        }
        return Ok(Some(ControlCommand::Rename {
            title: title.to_string(),
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
    if matches_surface_alias(&name, surface, "tree", &["树", "分支"]) {
        let turn_id = rest.trim();
        return Ok(Some(ControlCommand::Tree {
            turn_id: (!turn_id.is_empty()).then(|| turn_id.to_string()),
        }));
    }
    if matches_surface_alias(&name, surface, "goal", &["目标"]) {
        return super::goal::parse_goal_command(rest)
            .map(ControlCommand::Goal)
            .map(Some);
    }
    // 子智能体列表与留言命令只在 REPL 提供：网关会话不注册 subagent 工具
    if surface == ControlSurface::Repl && name == "subagents" {
        if !rest.trim().is_empty() {
            bail!(t(
                "subagents command does not accept arguments",
                "subagents 命令不接受参数"
            ));
        }
        return Ok(Some(ControlCommand::Subagents));
    }
    if surface == ControlSurface::Repl && name == "msg" {
        return parse_subagent_message_command(rest).map(Some);
    }
    Ok(None)
}

/// 解析子智能体留言命令参数。
///
/// 首个词是子智能体 ID（`subagent_` 前缀）或列表序号（纯数字）时作为目标，
/// 其余文本为消息；否则整段文本都是消息，目标由 REPL 依据当前查看的
/// 子智能体或唯一存活的子智能体推断。
///
/// 参数:
/// - `input`: 命令参数文本
///
/// 返回:
/// - 解析后的留言命令
fn parse_subagent_message_command(input: &str) -> Result<ControlCommand> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        bail!(t(
            "usage: /msg [id|index] <message>",
            "用法：/msg [ID|序号] <消息>"
        ));
    }
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let first = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or("").trim();
    let is_target = first.starts_with("subagent_") || first.chars().all(|ch| ch.is_ascii_digit());
    if is_target && !rest.is_empty() {
        Ok(ControlCommand::SubagentMessage {
            target: Some(first.to_string()),
            message: rest.to_string(),
        })
    } else {
        Ok(ControlCommand::SubagentMessage {
            target: None,
            message: trimmed.to_string(),
        })
    }
}

/// 拆分斜杠命令名称和参数。
///
/// 参数:
/// - `input`: 原始输入文本
///
/// 返回:
/// - 命令名和参数文本
/// 解析 `/context` 后的策略参数。
///
/// 参数:
/// - `rest`: 命令余下文本
///
/// 返回:
/// - 无参数时为空；`reset` 清除覆盖；否则为比例与可选预留
fn parse_context_policy_update(rest: &str) -> Result<Option<ContextPolicyUpdate>> {
    let rest = rest.trim();
    if rest.is_empty() {
        return Ok(None);
    }
    if matches!(rest, "reset" | "clear" | "默认" | "重置") {
        return Ok(Some(ContextPolicyUpdate::Reset));
    }
    let mut parts = rest.split_whitespace();
    let ratio_text = parts.next().unwrap_or_default();
    let reserve_text = parts.next();
    if parts.next().is_some() {
        bail!(t(
            "usage: /context [ratio] [reserve] | /context reset",
            "用法：/context [比例] [预留] 或 /context reset"
        ));
    }
    let ratio = crate::config::parse_compaction_ratio_text(ratio_text)?;
    let ratio_percent = (ratio * 100.0).round() as u32;
    let reserve = reserve_text.map(parse_reserve_tokens).transpose()?;
    Ok(Some(ContextPolicyUpdate::Set {
        ratio_percent,
        reserve,
    }))
}

/// 解析预留 token，支持 `8000` / `8k`。
///
/// 参数:
/// - `value`: 原始文本
///
/// 返回:
/// - token 数
fn parse_reserve_tokens(value: &str) -> Result<usize> {
    let trimmed = value.trim().trim_end_matches(['t', 'T']);
    let (number, scale) = if let Some(raw) = trimmed.strip_suffix(['k', 'K']) {
        (raw, 1_000usize)
    } else if let Some(raw) = trimmed.strip_suffix(['m', 'M']) {
        (raw, 1_000_000usize)
    } else {
        (trimmed, 1usize)
    };
    let parsed = number
        .parse::<f64>()
        .map_err(|_| anyhow::anyhow!("{}: {value}", t("invalid reserve", "无效预留")))?;
    if parsed < 0.0 || !parsed.is_finite() {
        bail!(t("invalid reserve", "无效预留"));
    }
    Ok((parsed * scale as f64).round() as usize)
}

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
    fn parses_context_policy_arguments() {
        assert_eq!(
            parse_control_command("/context", ControlSurface::Repl).unwrap(),
            Some(ControlCommand::Context { update: None })
        );
        assert_eq!(
            parse_control_command("/context reset", ControlSurface::Repl).unwrap(),
            Some(ControlCommand::Context {
                update: Some(ContextPolicyUpdate::Reset)
            })
        );
        assert_eq!(
            parse_control_command("/context 85 8k", ControlSurface::Repl).unwrap(),
            Some(ControlCommand::Context {
                update: Some(ContextPolicyUpdate::Set {
                    ratio_percent: 85,
                    reserve: Some(8_000)
                })
            })
        );
    }

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
    fn parses_subagents_list_command_only_in_repl() {
        assert_eq!(
            parse_control_command("/subagents", ControlSurface::Repl).unwrap(),
            Some(ControlCommand::Subagents)
        );
        assert!(parse_control_command("/subagents extra", ControlSurface::Repl).is_err());
        assert_eq!(
            parse_control_command("/subagents", ControlSurface::Gateway).unwrap(),
            None
        );
    }

    #[test]
    fn parses_subagent_message_with_optional_target() {
        // 首词为 ID 或序号时作为目标
        assert_eq!(
            parse_control_command("/msg subagent_1_2 请继续", ControlSurface::Repl).unwrap(),
            Some(ControlCommand::SubagentMessage {
                target: Some("subagent_1_2".to_string()),
                message: "请继续".to_string(),
            })
        );
        assert_eq!(
            parse_control_command("/msg 2 换个方案", ControlSurface::Repl).unwrap(),
            Some(ControlCommand::SubagentMessage {
                target: Some("2".to_string()),
                message: "换个方案".to_string(),
            })
        );
        // 普通文本整体作为消息,目标交由 REPL 推断
        assert_eq!(
            parse_control_command("/msg 请优先修复测试", ControlSurface::Repl).unwrap(),
            Some(ControlCommand::SubagentMessage {
                target: None,
                message: "请优先修复测试".to_string(),
            })
        );
        assert!(parse_control_command("/msg", ControlSurface::Repl).is_err());
        assert_eq!(
            parse_control_command("/msg hi", ControlSurface::Gateway).unwrap(),
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

    #[test]
    fn parses_rename_with_required_title() {
        assert_eq!(
            parse_control_command("/rename Sprint plan", ControlSurface::Repl).unwrap(),
            Some(ControlCommand::Rename {
                title: "Sprint plan".to_string()
            })
        );
        assert!(parse_control_command("/rename", ControlSurface::Repl).is_err());
        assert_eq!(
            parse_control_command("/重命名 本周计划", ControlSurface::Gateway).unwrap(),
            Some(ControlCommand::Rename {
                title: "本周计划".to_string()
            })
        );
    }
}
