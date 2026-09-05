use crate::i18n::text as t;

/// 构造 REPL 退出后的会话恢复命令。
///
/// 参数:
/// - `session_id`: 当前会话 ID
///
/// 返回:
/// - 形如 `sai resume <id>` 的命令文本
fn repl_resume_command(session_id: &str) -> String {
    format!("sai resume {session_id}")
}

/// 在 REPL 退出后向 stdout 打印会话恢复命令。
///
/// 参数:
/// - `session_id`: 当前会话 ID
///
/// 返回:
/// - 无
pub(super) fn print_repl_resume_hint(session_id: &str) {
    let command = repl_resume_command(session_id);
    println!();
    println!("{}", t("Resume this session with:", "恢复此会话："));
    println!("  {command}");
}

#[cfg(test)]
mod resume_hint_tests {
    use super::repl_resume_command;

    #[test]
    fn resume_command_matches_cli_resume_subcommand() {
        assert_eq!(
            repl_resume_command("5fed5f76-1823-43ab-be7a-35ba5b074cd1"),
            "sai resume 5fed5f76-1823-43ab-be7a-35ba5b074cd1"
        );
    }
}
