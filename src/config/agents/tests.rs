use super::*;

/// 验证统一 Agent 配置可以覆盖内置探索 Agent 并关闭主 Agent 注册。
#[test]
fn unified_agents_override_builtin_registration() {
    let mut config = crate::config::AppConfig::default();
    config.agents.push(AgentProfile {
        id: EXPLORE_AGENT_ID.to_string(),
        name: "项目探索".to_string(),
        description: "只查项目".to_string(),
        register_to_main: false,
        ..AgentProfile::default()
    });

    assert!(config
        .resolved_agent_profiles()
        .iter()
        .any(|profile| profile.id == EXPLORE_AGENT_ID && profile.name == "项目探索"));
    assert!(config
        .resolve_registered_agent(Some(EXPLORE_AGENT_ID))
        .is_none());
}

/// 验证 CLI 与 TUI 可以选择不同的默认 Agent。
#[test]
fn applies_surface_specific_default_agents() {
    let mut config = crate::config::AppConfig::default();
    config.agents.push(AgentProfile {
        id: "cli-agent".to_string(),
        name: "CLI".to_string(),
        system_prompt: "cli prompt".to_string(),
        ..AgentProfile::default()
    });
    config.agents.push(AgentProfile {
        id: "tui-agent".to_string(),
        name: "TUI".to_string(),
        system_prompt: "tui prompt".to_string(),
        ..AgentProfile::default()
    });
    config.cli_agent = Some("cli-agent".to_string());
    config.tui_agent = Some("tui-agent".to_string());

    let cli = apply_agent_override(config.clone(), None, AgentSurface::Cli).unwrap();
    let tui = apply_agent_override(config, None, AgentSurface::Tui).unwrap();
    assert_eq!(cli.system_prompt.as_deref(), Some("cli prompt"));
    assert_eq!(tui.system_prompt.as_deref(), Some("tui prompt"));
}

/// 验证终端入口强制使用内置内核，Web 仍保留外部内核。
#[test]
fn terminal_surfaces_force_native_engine_without_changing_web() {
    let mut config = crate::config::AppConfig::default();
    config.agent.engine = crate::config::AgentEngineKind::ClaudeCode;

    let cli = apply_agent_override(config.clone(), None, AgentSurface::Cli).unwrap();
    let tui = apply_agent_override(config.clone(), None, AgentSurface::Tui).unwrap();
    let web = apply_agent_override(config, None, AgentSurface::Web).unwrap();

    assert_eq!(cli.agent.engine, crate::config::AgentEngineKind::Native);
    assert_eq!(tui.agent.engine, crate::config::AgentEngineKind::Native);
    assert_eq!(web.agent.engine, crate::config::AgentEngineKind::ClaudeCode);
}

/// 内置代码 Agent 带有工程规范提示词；探索 Agent 为只读。
#[test]
fn builtin_agents_include_default_prompts() {
    let [cli, general, explore, plan, _gateway] = builtin_agent_profiles();
    assert!(cli.system_prompt.contains("Sai"));
    assert!(cli.enabled_tools.is_empty());
    assert!(general.system_prompt.contains("核心铁律"));
    assert!(!general.enabled_tools.is_empty());
    assert!(explore.system_prompt.contains("只读"));
    assert!(!explore.enabled_tools.is_empty());
    assert!(plan.system_prompt.contains("Plan"));
    assert!(!plan.enabled_tools.is_empty());
}

/// 默认入口：CLI 助手全量；TUI/Web 代码 Agent；网关专用。
#[test]
fn default_surfaces_use_cli_and_code_agents() {
    let mut config = crate::config::AppConfig::default();
    crate::config::ensure_surface_agent_defaults(&mut config);
    assert_eq!(config.cli_agent.as_deref(), Some(CLI_AGENT_ID));
    assert_eq!(config.tui_agent.as_deref(), Some(GENERAL_AGENT_ID));
    assert_eq!(config.default_agent.as_deref(), Some(GENERAL_AGENT_ID));
    assert_eq!(config.gateway_agent.as_deref(), Some(GATEWAY_AGENT_ID));
    let cli = apply_agent_override(config.clone(), None, AgentSurface::Cli).unwrap();
    let cli_runtime = cli.agent_runtime.expect("CLI 保留通配符延迟集合");
    assert!(cli_runtime.enabled_tools.is_empty(), "CLI 应继承全量工具");
    assert_eq!(
        cli_runtime.deferred_tools,
        vec![crate::config::DEFERRED_ALL_NON_BASE.to_string()],
        "CLI 非基础工具应按需 load"
    );
    assert!(cli.system_prompt.as_deref().unwrap_or("").contains("Sai"));
    assert!(cli.load_instruction_files);
    let tui = apply_agent_override(config.clone(), None, AgentSurface::Tui).unwrap();
    let runtime = tui.agent_runtime.expect("code agent whitelist");
    assert!(runtime.enabled_tools.iter().any(|t| t == "write_file"));
    assert!(runtime.enabled_tools.iter().any(|t| t == "str_replace"));
    assert!(runtime
        .enabled_tools
        .iter()
        .any(|t| t == "scientific_calculator"));
    assert!(runtime
        .enabled_tools
        .iter()
        .any(|t| t == "online_man_search"));
    assert!(runtime.enabled_tools.iter().any(|t| t == "ssh_list_hosts"));
    assert!(runtime.enabled_tools.iter().any(|t| t == "ssh_run_command"));
    assert!(!runtime.enabled_tools.iter().any(|t| t == "show_meme"));
    assert!(!runtime.enabled_tools.iter().any(|t| t == "calculate"));
    assert!(!runtime.enabled_tools.iter().any(|t| t == "man_page_search"));
    assert!(tui
        .system_prompt
        .as_deref()
        .unwrap_or("")
        .contains("核心铁律"));
    let gateway = apply_agent_override(config, None, AgentSurface::Gateway).unwrap();
    assert!(!gateway.load_instruction_files);
    let gateway_tools = gateway
        .agent_runtime
        .expect("gateway whitelist")
        .enabled_tools;
    assert!(gateway_tools.iter().any(|t| t == "get_weather"));
    assert!(gateway_tools.iter().any(|t| t == "get_exchange_rate"));
    assert!(gateway_tools.iter().any(|t| t == "query_deepseek_status"));
    assert!(gateway_tools.iter().any(|t| t == "online_man_get_page"));
    assert!(!gateway_tools.iter().any(|t| t == "query_weather"));
    assert!(!gateway_tools.iter().any(|t| t == "convert_exchange_rate"));
}

/// 验证内置 Agent 不会在配置重载时覆盖用户刚选择的模型。
#[test]
fn builtin_agent_keeps_user_selected_model_after_reload() {
    let mut config = crate::config::AppConfig::default();
    crate::config::ensure_surface_agent_defaults(&mut config);
    let provider_id = config.providers[0].id.clone();
    let selected_model = "user-selected-model";
    config
        .set_active_provider_model(&provider_id, selected_model)
        .unwrap();

    let resolved = apply_agent_override(config, None, AgentSurface::Tui).unwrap();

    assert_eq!(resolved.active_provider, provider_id);
    assert_eq!(
        resolved.provider(None).unwrap().default_model,
        selected_model
    );
}

/// 验证自定义 Agent 仍然可以固定供应商、模型和思考等级。
#[test]
fn custom_agent_can_pin_model_selection() {
    let mut config = crate::config::AppConfig::default();
    let provider_id = config.providers[0].id.clone();
    config.agents.push(AgentProfile {
        id: "pinned-agent".to_string(),
        name: "固定模型".to_string(),
        provider_id: provider_id.clone(),
        model: "pinned-model".to_string(),
        thinking_level: "high".to_string(),
        ..AgentProfile::default()
    });

    let resolved = apply_agent_override(config, Some("pinned-agent"), AgentSurface::Web).unwrap();
    let provider = resolved.provider(None).unwrap();
    assert_eq!(resolved.active_provider, provider_id);
    assert_eq!(provider.default_model, "pinned-model");
    assert_eq!(provider.thinking_level, "high");
}

#[test]
fn explore_and_plan_are_readonly_scoped() {
    let config = crate::config::AppConfig::default();
    let explore =
        apply_agent_override(config.clone(), Some(EXPLORE_AGENT_ID), AgentSurface::Web).unwrap();
    let tools = explore.agent_runtime.unwrap().enabled_tools;
    assert!(tools.iter().any(|t| t == "read_file"));
    assert!(!tools.iter().any(|t| t == "edit_file"));
    let plan = apply_agent_override(config, Some(PLAN_AGENT_ID), AgentSurface::Web).unwrap();
    let tools = plan.agent_runtime.unwrap().enabled_tools;
    assert!(tools.iter().any(|t| t == "web_search"));
    assert!(tools.iter().any(|t| t == "online_man_search"));
    assert!(!tools.iter().any(|t| t == "run_command"));
    assert!(!tools.iter().any(|t| t == "fetch_url"));
    assert!(plan.system_prompt.as_deref().unwrap_or("").contains("Plan"));
}

/// 白名单原样生效，不再对旧工具名做任何映射。
#[test]
fn the_whitelist_is_taken_verbatim() {
    let mut config = crate::config::AppConfig::default();
    config.agents.push(AgentProfile {
        id: "verbatim".to_string(),
        name: "原样".to_string(),
        enabled_tools: vec!["web_fetch".to_string(), "str_replace".to_string()],
        ..AgentProfile::default()
    });

    let resolved = apply_agent_override(config, Some("verbatim"), AgentSurface::Web).unwrap();
    let tools = resolved.agent_runtime.unwrap().enabled_tools;

    assert!(tools.iter().any(|tool| tool == "web_fetch"));
    assert!(tools.iter().any(|tool| tool == "str_replace"));
}

/// 验证旧子 Agent 档案会进入统一 Agent 列表并保留暴露状态。
#[test]
fn migrates_legacy_subagent_profiles_into_unified_agents() {
    let mut config = crate::config::AppConfig::default();
    config.subagent.profiles = vec![SubagentProfile {
        id: EXPLORE_AGENT_ID.to_string(),
        name: "旧探索".to_string(),
        description: "旧用途".to_string(),
        system_prompt: "旧提示".to_string(),
        provider_id: String::new(),
        model: String::new(),
        thinking_level: "high".to_string(),
        exposed: false,
    }];

    let profile = config
        .resolved_agent_profiles()
        .into_iter()
        .find(|profile| profile.id == EXPLORE_AGENT_ID)
        .unwrap();
    assert_eq!(profile.name, "旧探索");
    assert_eq!(profile.thinking_level, "high");
    assert!(!profile.register_to_main);
}
