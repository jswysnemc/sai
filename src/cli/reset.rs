use super::*;

/// 按 CLI 参数清空会话或助手记忆。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `scope`: 会话清理范围
/// - `memory`: 是否仅清理助手记忆
///
/// 返回:
/// - 操作是否成功
pub(super) fn run_reset(paths: &SaiPaths, scope: Option<&str>, memory: bool) -> Result<()> {
    if memory {
        println!("{}", clear_memory(paths, false)?);
        return Ok(());
    }
    let all = match scope {
        None => false,
        Some("all") => true,
        Some("全部") => true,
        Some(scope) => bail!("{}: {scope}", t("unknown reset scope", "未知 reset 范围")),
    };
    println!("{}", crate::control_commands::clear_state(paths, all)?);
    Ok(())
}
