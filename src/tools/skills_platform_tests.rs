use super::*;

/// 验证各平台使用符合约定的 Python 启动器顺序。
#[test]
fn python_launchers_match_platform_conventions() {
    let labels = python_launchers()
        .into_iter()
        .map(PythonLauncher::label)
        .collect::<Vec<_>>();
    #[cfg(windows)]
    assert_eq!(labels, vec!["py -3", "python", "python3"]);
    #[cfg(not(windows))]
    assert_eq!(labels, vec!["python3", "python"]);
}

/// 验证技能发现覆盖常用第三方智能体目录。
#[test]
fn third_party_skill_roots_cover_common_agent_paths() {
    let roots = third_party_skill_roots();
    let scopes: std::collections::BTreeSet<_> = roots.iter().map(|(scope, _)| *scope).collect();
    assert!(scopes.contains("claude") || scopes.contains("project_claude"));
    assert!(scopes.contains("codex") || scopes.contains("project_codex"));
    assert!(
        scopes.contains("agents") || scopes.contains("project_agents") || scopes.contains("agent")
    );
    assert!(
        scopes.contains("opencode")
            || scopes.contains("opencode_home")
            || scopes.contains("project_opencode")
    );
}
