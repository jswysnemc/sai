use super::*;
use crate::config::AgentRuntimeOverride;
use crate::gateways::channel_context::ChannelContext;
use crate::gateways::session::ensure_gateway_session;
use crate::runner::{SessionOwner, SessionPresenceGuard, SubmissionSource};
use serde_json::{json, Value};

/// 【微信网关】【配置回归】无参数，返回无；运行期副本开启通信而不改变原始配置。
#[test]
fn gateway_configuration_does_not_change_other_sessions() {
    let original = AppConfig::default();
    let mut runtime = original.clone();
    configure(&mut runtime);
    assert!(runtime.mesh.cross_session);
    assert!(!original.mesh.cross_session);
    assert!(runtime.agent_runtime.is_none());
}

/// 【微信网关】【白名单回归】无参数，返回无；固定通信工具组可穿过限制且不重复添加。
#[test]
fn restrictive_profiles_keep_the_fixed_mesh_group() {
    for exclusive in [false, true] {
        let mut config = AppConfig::default();
        config.agent_runtime = Some(AgentRuntimeOverride {
            enabled_tools: vec!["read_file".into(), "mesh_send".into()],
            exclusive,
            ..Default::default()
        });
        configure(&mut config);
        configure(&mut config);
        let runtime = config.agent_runtime.unwrap();
        assert_eq!(runtime.exclusive, exclusive);
        assert_eq!(runtime.enabled_tools.len(), TOOL_NAMES.len() + 1);
        assert_eq!(runtime.enabled_tools[0], "read_file");
        for name in TOOL_NAMES {
            assert_eq!(
                runtime
                    .enabled_tools
                    .iter()
                    .filter(|item| *item == name)
                    .count(),
                1
            );
        }
    }
}

/// 【微信网关】【空白名单回归】无参数，返回无；非独占空列表继续表示不限制工具。
#[test]
fn unrestricted_profile_stays_unrestricted() {
    let mut config = AppConfig::default();
    config.agent_runtime = Some(AgentRuntimeOverride::default());
    configure(&mut config);
    assert!(config.agent_runtime.unwrap().enabled_tools.is_empty());
}

/// 【微信网关】【加载回归】无参数，返回无；渠道预加载工具通过渐进执行门禁。
#[test]
fn preloaded_mesh_tools_pass_progressive_visibility_checks() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.agent_runtime = Some(AgentRuntimeOverride {
        deferred_tools: TOOL_NAMES.into_iter().map(str::to_string).collect(),
        ..Default::default()
    });
    configure(&mut config);
    let mut registry = ToolRegistry::new();
    let gateway =
        ensure_gateway_session(&paths, &ChannelContext::weixin("user".into(), None)).unwrap();
    register(&mut registry, &paths, &gateway).unwrap();
    let mut visibility = crate::agent::ToolVisibility::from_config(&config);
    assert!(visibility.is_progressive());
    assert!(visibility.requires_load("mesh_send"));
    let mut channel = crate::runner::ChannelSubmission::new("weixin");
    for name in TOOL_NAMES {
        channel = channel.with_extra_loaded_tool(name);
    }
    visibility.restore_loaded_tools(&registry, &channel.extra_loaded_tools);
    for name in TOOL_NAMES {
        assert!(visibility.is_loaded(name));
        assert!(visibility.is_visible(name));
    }
}

/// 【微信网关】【投递回归】无参数，返回无；真实会话注册经过过滤与身份重绑后可跨工作区投递。
#[tokio::test]
async fn gateway_probes_and_sends_with_its_actual_session_identity() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let workspace = root.path().join("local-workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    crate::runtime_cwd::scope(workspace.clone(), async {
        let local =
            crate::state::create_session_for_workspace(&paths, &workspace, Some("local")).unwrap();
        let local_dir = crate::state::locate_session_dirs(&paths, &local.id)
            .unwrap()
            .1;
        let _presence =
            SessionPresenceGuard::register(&local_dir, &local.id, SessionOwner::Repl).unwrap();
        let gateway =
            ensure_gateway_session(&paths, &ChannelContext::weixin("user".into(), None)).unwrap();
        let gateway_dir = crate::state::locate_session_dirs(&paths, &gateway)
            .unwrap()
            .1;
        let mut config = AppConfig::default();
        config.agent_runtime = Some(AgentRuntimeOverride {
            enabled_tools: vec!["read_file".into()],
            exclusive: true,
            ..Default::default()
        });
        configure(&mut config);
        let mut registry = ToolRegistry::new();
        register(&mut registry, &paths, &gateway).unwrap();
        let mut registry =
            crate::runner::apply_enabled_tools_filter(registry, &config, SubmissionSource::Gateway)
                .unwrap();
        registry.set_permission_profile(crate::permission::PermissionProfile::new(
            crate::permission::PermissionProfileMode::Yolo,
            workspace.clone(),
            None,
        ));
        registry.rebind_interactive_session(&config, &paths, &gateway, &gateway_dir);
        for name in TOOL_NAMES {
            assert!(registry.contains(name));
        }
        let probe: Value = serde_json::from_str(
            &registry
                .call("session_probe", r#"{"scope":"all"}"#)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(probe["self"]["session_id"], gateway);
        assert_eq!(probe["sessions"][0]["id"], local.id);
        let result: Value = serde_json::from_str(
            &registry
                .call(
                    "mesh_send",
                    &json!({
                        "to": format!("session:{}", local.id),
                        "text": "forwarded instruction",
                        "correlation_id": "weixin-thread",
                    })
                    .to_string(),
                )
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(result["ok"], true);
        assert_eq!(result["from"], format!("session:{gateway}"));
        let pending = crate::tools::mesh::next_pending(&local_dir, &local.id).unwrap();
        assert_eq!(pending.text, "forwarded instruction");
        assert_eq!(pending.from, format!("session:{gateway}"));
        assert_eq!(pending.reply_to, Some(pending.from.clone()));
        assert_eq!(pending.correlation_id.as_deref(), Some("weixin-thread"));

        // 1. 【微信网关】【权限隔离】普通会话继续拒绝向网关会话投递
        let mut ordinary = ToolRegistry::new();
        crate::tools::register_interactive_tools(
            &mut ordinary,
            &AppConfig::default(),
            &paths,
            local_dir.display().to_string(),
            local.id.clone(),
        );
        let error = ordinary
            .call(
                "mesh_send",
                &json!({"to": format!("session:{gateway}"), "text": "hi"}).to_string(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("outside this session"));
    })
    .await;
}

/// 【微信网关】【提示词回归】无参数，返回无；说明使用真实发送工具并区分投递和完成。
#[test]
fn prompt_guides_probe_before_forwarding() {
    let prompt = super::super::prompt::channel_prompt();
    for text in [
        "session_probe",
        "mesh_send",
        "correlation_id",
        "不代表任务已经完成",
        "仅改变目标工作区",
    ] {
        assert!(prompt.contains(text), "missing guidance: {text}");
    }
}
