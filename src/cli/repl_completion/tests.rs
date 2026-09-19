use super::*;

/// 命令补全后继续提供参数，确认参数仅改输入，完整输入再次回车才执行
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
    let second = accept(&first, first.chars().count(), 1, false, true).unwrap();
    assert_eq!(second, "/context reset");
    assert_eq!(
        accept(&second, second.chars().count(), 0, false, true),
        None
    );
    assert_eq!(accept("/help", 5, 0, false, true), None);
    assert_eq!(accept("/hel", 4, 0, false, true).as_deref(), Some("/help"));
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
