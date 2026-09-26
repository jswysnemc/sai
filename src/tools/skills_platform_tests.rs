use super::*;

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
