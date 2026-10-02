use crate::config::{
    apply_agent_override, seed_default_agent_profiles, AgentSurface, AppConfig,
};

/// 【网页读取测试】【预设升级】无参数；完整旧预设补齐原生读取，定制和独占白名单保持不变。
#[test]
fn web_fetch_is_available_in_default_and_unchanged_legacy_profiles() {
    for mut profile in seed_default_agent_profiles()
        .into_iter()
        .filter(|profile| profile.id != "cli")
    {
        assert!(profile.enabled_tools.iter().any(|name| name == "web_fetch"));
        profile.enabled_tools.retain(|name| name != "web_fetch");
        let mut config = AppConfig {
            agents: vec![profile.clone()],
            ..AppConfig::default()
        };
        let updated =
            apply_agent_override(config.clone(), Some(&profile.id), AgentSurface::Web).unwrap();
        assert!(updated
            .agent_runtime
            .unwrap()
            .enabled_tools
            .iter()
            .any(|name| name == "web_fetch"));
        config.agents[0].tools_exclusive = true;
        let updated =
            apply_agent_override(config.clone(), Some(&profile.id), AgentSurface::Web).unwrap();
        assert!(!updated
            .agent_runtime
            .unwrap()
            .enabled_tools
            .iter()
            .any(|name| name == "web_fetch"));
        config.agents[0].tools_exclusive = false;
        config.agents[0].enabled_tools = vec!["read_file".into()];
        let updated = apply_agent_override(config, Some(&profile.id), AgentSurface::Web).unwrap();
        assert_eq!(updated.agent_runtime.unwrap().enabled_tools, ["read_file"]);
    }
}
