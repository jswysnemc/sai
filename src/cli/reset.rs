use super::*;

/// 按 CLI 参数清空会话或助手记忆。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `scope`: 会话清理范围
/// - `memory`: 是否仅清理助手记忆
/// - `assume_yes`: 是否跳过破坏性操作确认
///
/// 返回:
/// - 操作是否成功
pub(super) fn run_reset(
    paths: &SaiPaths,
    scope: Option<&str>,
    memory: bool,
    assume_yes: bool,
) -> Result<()> {
    if memory {
        if !confirm::confirm_destructive(t("clear assistant memory", "清空助手记忆"), assume_yes)?
        {
            return Ok(());
        }
        println!("{}", clear_memory(paths)?);
        return Ok(());
    }
    let all = match scope {
        None => false,
        Some("all") => true,
        Some("全部") => true,
        Some(scope) => bail!("{}: {scope}", t("unknown reset scope", "未知 reset 范围")),
    };
    // clear 与 clear all 一词之差代价悬殊，执行前必须确认
    let action = if all {
        t(
            "clear session history and all memories",
            "清空会话历史与全部记忆",
        )
    } else {
        t("clear current session history", "清空当前会话历史")
    };
    if !confirm::confirm_destructive(action, assume_yes)? {
        return Ok(());
    }
    println!("{}", crate::control_commands::clear_state(paths, all)?);
    Ok(())
}
