use super::*;

/// 【CodeMode】【混合声明】无参数；验证真实会话同时声明普通工具和脚本，且不复制业务说明。
#[test]
fn hybrid_session_shares_native_definitions_without_inline_duplicates() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let agent = agent(
        &paths,
        AgentMode::Yolo.with_codemode(true),
        AppConfig::default(),
        Arc::default(),
    );
    let definitions = agent.tool_visibility.definitions(&agent.tools);
    assert!(definitions.iter().any(|item| item.function.name == "echo"));
    let script = definitions
        .iter()
        .find(|item| item.function.name == "codemode")
        .unwrap();
    assert!(!script.function.description.contains("Echo text"));
    assert!(!script.function.description.contains("Change state"));
    assert!(matches!(
        tool_gate::evaluate_tool_gate(
            &agent.tools,
            &agent.tool_visibility,
            &call("echo", json!({"text":"native"}))
        ),
        tool_gate::ToolGate::Proceed
    ));
}

/// 【CodeMode】【独占切换】无参数；验证独占方式必须显式选择，并可恢复混合或关闭脚本。
#[test]
fn exclusive_script_mode_is_explicit_and_round_trips() {
    let hybrid = AgentMode::Yolo.with_codemode(true);
    let only = hybrid
        .command("/code-model only")
        .expect("explicit only command");
    assert_ne!(only, hybrid);
    assert_eq!(AgentMode::parse(Some(only.key())).unwrap(), only);
    assert_eq!(only.command("/code-model on"), Some(hybrid));
    assert_eq!(only.command("/code-model off"), Some(AgentMode::Yolo));
    let live = LiveAgentMode::new(only, None);
    assert_eq!(live.get(), only);
}

/// 【CodeMode】【共享加载】无参数；验证串行及只读并发入口都拒绝未加载工具，加载后可执行。
#[tokio::test]
async fn nested_calls_share_deferred_loading_with_native_calls() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.agent_runtime = Some(crate::config::AgentRuntimeOverride {
        deferred_tools: vec!["echo".into()],
        ..Default::default()
    });
    let mut agent = agent(
        &paths,
        AgentMode::Yolo.with_codemode(true),
        config,
        Arc::default(),
    );
    assert!(agent
        .codemode_readonly_call(&nested("echo", json!({"text":"hidden"})))
        .is_none());
    assert!(agent
        .execute_codemode_call(
            nested("echo", json!({"text":"hidden"})),
            &[],
            &mut 0,
            &mut |_| Ok(())
        )
        .await
        .is_err());
    let result = agent
        .tool_visibility
        .load_from_arguments(
            &agent.tools,
            r#"{"type":"tool","keywords":["echo"]}"#,
            &agent.config,
            &paths,
        )
        .unwrap();
    assert!(result.contains("Echo text"));
    assert!(result.contains("codemode"));
    assert!(agent
        .codemode_readonly_call(&nested("echo", json!({"text":"loaded"})))
        .is_some());
    let output = agent
        .execute_codemode_call(
            nested("echo", json!({"text":"loaded"})),
            &[],
            &mut 0,
            &mut |_| Ok(()),
        )
        .await
        .unwrap();
    assert_eq!(output, json!({"text":"loaded"}));
    assert!(agent
        .tool_visibility
        .codemode_catalog(&agent.tools)
        .iter()
        .any(|entry| entry.name == "echo"));
    agent
        .tool_visibility
        .restore_loaded_tools(&agent.tools, &[]);
    assert!(!agent
        .tool_visibility
        .codemode_catalog(&agent.tools)
        .iter()
        .any(|entry| entry.name == "echo"));
    assert!(agent
        .codemode_readonly_call(&nested("echo", json!({"text":"after-compaction"})))
        .is_none());
}

/// 【CodeMode】【Jev共享】无参数；验证预选仅注入一次定义，两条调用路径共享暴露状态。
#[test]
fn jev_exposure_does_not_duplicate_script_catalog_descriptions() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.jev.routing.enabled = true;
    let mut agent = agent(
        &paths,
        AgentMode::Yolo.with_codemode(true),
        config,
        Arc::default(),
    );
    let selection = crate::jev::Selection {
        tools: vec!["echo".into()],
        prompts: Vec::new(),
        skills: Vec::new(),
    };
    assert!(!agent
        .tool_visibility
        .codemode_catalog(&agent.tools)
        .iter()
        .any(|entry| entry.name == "echo"));
    let first = agent
        .tool_visibility
        .announce_selection(&agent.tools, &selection, &agent.config, &paths)
        .unwrap();
    assert!(first.contains("Echo text"));
    assert!(first.contains("codemode"));
    let second = agent
        .tool_visibility
        .announce_selection(&agent.tools, &selection, &agent.config, &paths)
        .unwrap();
    assert!(!second.contains("Echo text"));
    assert!(agent.tool_visibility.is_script_tool_exposed("echo"));
    for definition in agent.tool_visibility.definitions(&agent.tools) {
        assert!(!definition.function.description.contains("Echo text"));
    }
}

/// 【CodeMode】【独占加载】无参数；验证发现入口可用，已加载定义不再内联到脚本说明。
#[test]
fn exclusive_mode_keeps_loading_and_emits_loaded_schemas_once() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.agent_runtime = Some(crate::config::AgentRuntimeOverride {
        deferred_tools: vec!["echo".into()],
        ..Default::default()
    });
    let mode = AgentMode::Yolo.with_code_mode(CodeMode::Only);
    let mut agent = agent(&paths, mode, config, Arc::default());
    let definitions = agent.tool_visibility.definitions(&agent.tools);
    assert!(definitions
        .iter()
        .any(|definition| definition.function.name == "load"));
    assert!(!definitions
        .iter()
        .any(|definition| definition.function.name == "invoke_tool"));
    let script = definitions
        .iter()
        .find(|definition| definition.function.name == "codemode")
        .unwrap();
    assert!(!script.function.description.contains("Echo text"));
    assert!(script.function.description.contains("Change state"));
    agent
        .tool_visibility
        .load_from_arguments(
            &agent.tools,
            r#"{"type":"tool","keywords":["echo"]}"#,
            &agent.config,
            &paths,
        )
        .unwrap();
    let definitions = agent.tool_visibility.definitions(&agent.tools);
    assert!(!definitions
        .iter()
        .filter(|definition| definition.function.name == "codemode")
        .any(|definition| definition.function.description.contains("Echo text")));
    assert!(agent
        .codemode_readonly_call(&nested("echo", json!({"text":"loaded"})))
        .is_some());
    assert!(matches!(
        tool_gate::evaluate_tool_gate(
            &agent.tools,
            &agent.tool_visibility,
            &call("echo", json!({"text":"direct"}))
        ),
        tool_gate::ToolGate::Reject(_)
    ));
}

/// 【CodeMode】【锚定适配】无参数；验证脚本使用原生别名定义，参数转换后仍校验真实工具。
#[test]
fn anchored_tools_share_names_and_argument_translation() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut agent = agent(
        &paths,
        AgentMode::Yolo.with_codemode(true),
        AppConfig::default(),
        Arc::default(),
    );
    agent.tools.register(ToolSpec::new("read_file", "Read fixture", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}), |_| async { Ok("fixture".into()) }));
    agent.tool_visibility =
        tool_visibility::ToolVisibility::from_config_with_anchor(&agent.config, true, false);
    agent.tool_visibility.set_session_mode(agent.mode());
    let catalog = agent.tool_visibility.codemode_catalog(&agent.tools);
    assert!(catalog
        .iter()
        .any(|entry| entry.name == "str_replace_editor"));
    assert!(!catalog.iter().any(|entry| entry.name == "read_file"));
    let (_, call) = agent
        .prepare_codemode_call(&nested(
            "str_replace_editor",
            json!({"command":"view","path":root.path().join("fixture.txt")}),
        ))
        .unwrap();
    assert_eq!(call.function.name, "read_file");
    assert!(agent
        .prepare_codemode_call(&nested("read_file", json!({"path":"fixture.txt"})))
        .is_err());
}
