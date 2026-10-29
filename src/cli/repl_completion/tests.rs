use super::*;

/// 【终端】【命令补全】Tab 补全参数，完整命令回车直接执行；参数与返回值均无。
#[test]
fn command_argument_acceptance_is_separate_from_submission() {
    let first = accept("/cont", 5, 0, false, false).unwrap();
    assert_eq!(first, "/context ");
    let candidates = visible_repl_command_suggestions(&first, false);
    assert_eq!(
        candidates
            .iter()
            .map(|item| item.command)
            .collect::<Vec<_>>(),
        ["/context edit", "/context reset"]
    );
    assert_eq!(accept(&first, first.chars().count(), 1, false, true), None);
    let second = accept(&first, first.chars().count(), 1, false, false).unwrap();
    assert_eq!(second, "/context reset");
    assert_eq!(
        accept(&second, second.chars().count(), 0, false, true),
        None
    );
    assert_eq!(accept("/help", 5, 0, false, true), None);
    assert_eq!(accept("/hel", 4, 0, false, true).as_deref(), Some("/help"));
}

/// 【终端】【命令提交】完整基础命令不受候选序号、尾部空格和运行状态影响；无参数或返回值。
#[test]
fn enter_never_changes_a_complete_command_to_a_subcommand() {
    for command in [
        "/context",
        "/CONTEXT ",
        "/context   ",
        "/goal ",
        "/clear ",
        "/jev ",
    ] {
        for streaming in [false, true] {
            for selected in [0, 1, usize::MAX] {
                assert_eq!(
                    accept(command, command.chars().count(), selected, streaming, true),
                    None
                );
            }
        }
    }
}

/// 运行时参数禁用策略与执行端一致，光标在中间不覆盖尾部
#[test]
fn completion_respects_policy_and_cursor() {
    let suggestions = arguments("/context ", true);
    assert!(suggestions[0].disabled);
    assert!(!suggestions[1].disabled);
    assert_eq!(accept("/context e", 10, 0, true, false), None);
    assert_eq!(accept("/context reset", 3, 0, false, false), None);
    assert!(arguments("/context 85% 50k", false).is_empty());
}
