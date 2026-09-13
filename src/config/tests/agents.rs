/// 新装机器的内置 Agent 默认把非基础工具划入 load 态。
#[test]
fn builtin_agents_defer_non_base_tools_by_default() {
    let profiles = crate::config::seed_default_agent_profiles();
    let code = profiles
        .iter()
        .find(|profile| profile.id == crate::config::GENERAL_AGENT_ID)
        .expect("代码 Agent 必须存在");

    // 基础文件与命令工具保持初始可见
    assert!(!code.deferred_tools.iter().any(|tool| tool == "read_file"));
    assert!(!code.deferred_tools.iter().any(|tool| tool == "edit_file"));
    // 检索类工具是长程编程的高频入口，同样保持可见
    assert!(!code.deferred_tools.iter().any(|tool| tool == "web_search"));
    // 低频重量级工具交给模型按需 load
    assert!(code
        .deferred_tools
        .iter()
        .any(|tool| tool == "ssh_list_hosts"));

    let cli = profiles
        .iter()
        .find(|profile| profile.id == crate::config::CLI_AGENT_ID)
        .expect("CLI Agent 必须存在");

    // CLI 全量开放且工具名不可穷举，用通配符表达同一策略
    assert_eq!(
        cli.deferred_tools,
        vec![crate::config::DEFERRED_ALL_NON_BASE.to_string()]
    );
}
