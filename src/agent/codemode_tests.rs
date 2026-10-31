use super::*;
use crate::config::CodeMode;
use crate::llm::{ToolCall, ToolCallFunction};
use crate::tools::ToolSpec;
use sai_codemode::protocol::WorkerEvent;
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

/// 【CodeMode】【测试注册表】参数 counter 为执行计数器，返回包含读写工具的注册表。
fn registry(counter: Arc<AtomicUsize>) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(ToolSpec::new(
        "echo",
        "Echo text",
        json!({
            "type":"object", "properties":{"text":{"type":"string"}},
            "required":["text"], "additionalProperties":false
        }),
        |args| async move { Ok(json!({"text":args["text"]}).to_string()) },
    ));
    registry.register(
        ToolSpec::new(
            "change",
            "Change state",
            crate::tools::empty_parameters(),
            move |_| {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok("changed".into())
                }
            },
        )
        .writes(),
    );
    registry
}

/// 【CodeMode】【测试会话】使用 paths/mode/config/counter 创建无网络测试 Agent，返回会话实例。
fn agent(
    paths: &SaiPaths,
    mode: AgentMode,
    mut config: AppConfig,
    counter: Arc<AtomicUsize>,
) -> Agent {
    config.skills.enabled = false;
    config.load_instruction_files = false;
    let state = StateStore::new(paths).unwrap();
    let client = OpenAiCompatibleClient::from_config(&config, paths).unwrap();
    let mut registry = registry(counter);
    registry.set_permission_profile(crate::permission::PermissionProfile::new(
        mode.permission_profile_mode(),
        paths.state_dir.clone(),
        None,
    ));
    Agent::new(config, paths, state, client, registry, mode).unwrap()
}

/// 【CodeMode】【测试调用】把 name/arguments 构造为供应商调用，返回调用对象。
fn call(name: &str, arguments: serde_json::Value) -> ToolCall {
    ToolCall {
        id: "test-call".into(),
        kind: "function".into(),
        function: ToolCallFunction {
            name: name.into(),
            arguments: arguments.to_string(),
        },
    }
}

/// 【CodeMode】【嵌套测试】把 name/arguments 构造为工作进程事件，返回请求。
fn nested(name: &str, arguments: serde_json::Value) -> WorkerEvent {
    WorkerEvent::Call {
        id: 1,
        name: name.into(),
        arguments,
    }
}

#[test]
fn on_only_off_have_consistent_declarations_and_execution_gates() {
    for mode in [CodeMode::On, CodeMode::Only, CodeMode::Off] {
        let mut config = AppConfig::default();
        config.codemode.mode = mode;
        let mut registry = registry(Arc::default());
        crate::tools::codemode::register(&mut registry, &config.codemode);
        let visibility = tool_visibility::ToolVisibility::from_config(&config);
        let definitions = visibility.definitions(&registry);
        assert_eq!(
            definitions
                .iter()
                .any(|item| item.function.name == "codemode"),
            mode != CodeMode::Off
        );
        assert_eq!(
            definitions.iter().any(|item| item.function.name == "echo"),
            mode != CodeMode::Only
        );
        let direct = tool_gate::evaluate_tool_gate(
            &registry,
            &visibility,
            &call("echo", json!({"text":"a"})),
        );
        assert_eq!(
            matches!(direct, tool_gate::ToolGate::Proceed),
            mode != CodeMode::Only
        );
        if mode == CodeMode::Only {
            assert_eq!(definitions.len(), 1);
            for name in [
                crate::tools::LOAD_NAME,
                crate::tools::INVOKE_NAME,
                "dsh_bash",
            ] {
                assert!(tool_invocation::resolve_execution_call(
                    &visibility,
                    &call(name, json!({}))
                )
                .is_err());
            }
        }
    }
}

#[test]
fn codemode_is_available_under_progressive_and_anchor_catalogs() {
    for anchored in [false, true] {
        let mut config = AppConfig::default();
        config.agent_runtime = Some(crate::config::AgentRuntimeOverride {
            deferred_tools: vec!["*".into()],
            ..Default::default()
        });
        let mut registry = registry(Arc::default());
        crate::tools::codemode::register(&mut registry, &config.codemode);
        let visibility =
            tool_visibility::ToolVisibility::from_config_with_anchor(&config, anchored, false);
        assert!(visibility.is_visible("codemode"));
        assert!(!visibility.requires_load("codemode"));
        assert!(visibility
            .definitions(&registry)
            .iter()
            .any(|item| item.function.name == "codemode"));
        assert!(tool_invocation::resolve_execution_call(
            &visibility,
            &call("codemode", json!({"code":"return 1"}))
        )
        .is_ok());
    }
}

#[test]
fn dynamic_catalog_respects_current_registry_and_inline_budget() {
    let mut registry = registry(Arc::default());
    let mut config = crate::config::CodeModeConfig::default();
    crate::tools::codemode::register(&mut registry, &config);
    let first = crate::tools::codemode::definition(&registry, &config).unwrap();
    assert!(first.function.description.contains("Echo text"));
    registry.remove("echo");
    let next = crate::tools::codemode::definition(&registry, &config).unwrap();
    assert!(!next.function.description.contains("Echo text"));
    assert!(!crate::tools::codemode::catalog(&registry)
        .iter()
        .any(|entry| entry.name == "echo" || entry.name == "codemode"));
    config.inline_budget = 0;
    assert!(!crate::tools::codemode::definition(&registry, &config)
        .unwrap()
        .function
        .description
        .contains("Change state"));
}

#[tokio::test]
async fn nested_tools_validate_and_do_not_bypass_whitelist_or_plan_mode() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let counter = Arc::new(AtomicUsize::new(0));
    let mut agent = agent(
        &paths,
        AgentMode::Plan,
        AppConfig::default(),
        counter.clone(),
    );
    let mut events = Vec::new();
    let mut callback = |event| {
        events.push(event);
        Ok(())
    };
    let output = agent
        .execute_codemode_call(
            nested("echo", json!({"text":"hello"})),
            &[],
            &mut 0,
            &mut callback,
        )
        .await
        .unwrap();
    assert_eq!(output, json!({"text":"hello"}));
    for (name, arguments) in [
        ("change", json!({})),
        ("unknown", json!({})),
        ("echo", json!({"invalid":1})),
        ("codemode", json!({"code":"return 1"})),
    ] {
        assert!(agent
            .execute_codemode_call(nested(name, arguments), &[], &mut 0, &mut callback)
            .await
            .is_err());
    }
    assert_eq!(counter.load(Ordering::SeqCst), 0);
    assert!(events
        .iter()
        .all(|event| !matches!(event, AgentEvent::ToolResult { .. })));
}

#[tokio::test]
async fn nested_write_requests_real_permission_and_denial_prevents_execution() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let counter = Arc::new(AtomicUsize::new(0));
    let mut agent = agent(
        &paths,
        AgentMode::Audited,
        AppConfig::default(),
        counter.clone(),
    );
    let mut requested = 0;
    let output = agent
        .execute_codemode_call(nested("change", json!({})), &[], &mut 0, &mut |event| {
            if let AgentEvent::PermissionRequested(request) = event {
                requested += 1;
                assert_eq!(request.tool, "change");
                crate::permission::decide_permission(
                    &request.id,
                    crate::permission::PermissionDecision::Deny {
                        reply: Some("denied by test".into()),
                    },
                )?;
            }
            Ok(())
        })
        .await
        .unwrap_err();
    assert!(output.to_string().contains("denied by test"));
    assert_eq!(requested, 1);
    assert_eq!(counter.load(Ordering::SeqCst), 0);
    let result = agent
        .execute_codemode_call(nested("change", json!({})), &[], &mut 0, &mut |event| {
            if let AgentEvent::PermissionRequested(request) = event {
                crate::permission::decide_permission(
                    &request.id,
                    crate::permission::PermissionDecision::allow_once(),
                )?;
            }
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(result, json!("changed"));
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn replacing_registry_keeps_wrapper_without_stale_nested_tools() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut agent = agent(
        &paths,
        AgentMode::Yolo,
        AppConfig::default(),
        Arc::default(),
    );
    agent.replace_tools(ToolRegistry::new());
    assert!(agent.tools.contains("codemode"));
    assert!(!agent.tools.contains("echo"));
    assert!(agent
        .execute_codemode_call(
            nested("echo", json!({"text":"x"})),
            &[],
            &mut 0,
            &mut |_| Ok(())
        )
        .await
        .is_err());
}

#[test]
fn config_roundtrip_and_limits_are_validated() {
    let mut config = AppConfig::default();
    assert_eq!(config.codemode.mode, CodeMode::On);
    let value = serde_json::to_value(&config).unwrap();
    let restored: AppConfig = serde_json::from_value(value).unwrap();
    assert_eq!(restored.codemode.timeout_ms, 60_000);
    config.codemode.timeout_ms = 0;
    assert!(config.validate().is_err());
    assert!(
        serde_json::from_str::<crate::config::CodeModeConfig>("{\"mode\":\"invalid\"}").is_err()
    );
}

#[tokio::test]
async fn concurrent_readonly_tools_preserve_workspace_scope() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let workspace = root.path().join("different-workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let mut agent = agent(
        &paths,
        AgentMode::Yolo,
        AppConfig::default(),
        Arc::default(),
    );
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    agent.tools.register(ToolSpec::new(
        "workspace",
        "Current workspace",
        crate::tools::empty_parameters(),
        move |_| {
            let barrier = barrier.clone();
            async move {
                barrier.wait().await;
                Ok(crate::runtime_cwd::current_dir()?.display().to_string())
            }
        },
    ));
    let (progress, _receiver) = tokio::sync::mpsc::channel(128);
    let mut jobs = tokio::task::JoinSet::new();
    crate::runtime_cwd::scope(workspace.clone(), async {
        for id in 1..=2 {
            let mut call = call("workspace", json!({}));
            call.id = format!("parallel-{id}");
            agent.spawn_codemode_readonly(id, call, &mut jobs, progress.clone());
        }
    })
    .await;
    for _ in 0..2 {
        let (_, _, result) =
            tokio::time::timeout(std::time::Duration::from_secs(2), jobs.join_next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        assert_eq!(result.unwrap().content, workspace.display().to_string());
    }
}

#[test]
fn exclusive_whitelist_requires_explicit_codemode_and_never_adds_nested_tools() {
    for allowed in [false, true] {
        let mut config = AppConfig::default();
        config.agent_runtime = Some(crate::config::AgentRuntimeOverride {
            enabled_tools: if allowed {
                vec!["codemode".into()]
            } else {
                vec![]
            },
            exclusive: true,
            ..Default::default()
        });
        let mut registry = ToolRegistry::new();
        crate::tools::codemode::register_for_agent(&mut registry, &config);
        assert_eq!(registry.contains("codemode"), allowed);
        assert!(crate::tools::codemode::catalog(&registry).is_empty());
    }
}
