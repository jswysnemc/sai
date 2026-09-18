use super::*;

/// 交互式会话注册表里能直接拿到全部网格工具。
#[tokio::test]
async fn interactive_registry_exposes_all_mesh_tools() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace-a");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (session, state_dir) = session_in(&paths, &cwd, "owner");
        let config = crate::config::AppConfig::load_or_default(&paths).unwrap();
        let mut registry = ToolRegistry::new();

        crate::tools::register_interactive_tools(
            &mut registry,
            &config,
            &paths,
            state_dir.display().to_string(),
            session.id.clone(),
        );

        assert!(registry.contains("session_probe"));
        assert!(registry.contains("agent_probe"));
        assert!(registry.contains("mesh_send"));
        assert!(registry.contains("session_create"));
        assert!(registry.contains("session_activate"));
        assert!(
            !registry.contains("mesh_reply"),
            "results go back through mesh_send; mesh_reply is removed"
        );
        assert!(
            !registry.contains("mesh_recv"),
            "incoming mesh messages are queued as active receipts; there is no recv tool"
        );
        // 探测是只读的，不能要求写权限确认
        assert_eq!(
            registry.permission("session_probe").unwrap(),
            crate::tools::ToolPermission::ReadOnly
        );
        assert_eq!(
            registry.permission("agent_probe").unwrap(),
            crate::tools::ToolPermission::ReadOnly
        );
        // 创建与激活都会落盘改状态，审计模式下必须走写权限确认
        assert_eq!(
            registry.permission("session_create").unwrap(),
            crate::tools::ToolPermission::Writes
        );
        assert_eq!(
            registry.permission("session_activate").unwrap(),
            crate::tools::ToolPermission::Writes
        );
    })
    .await;
}

/// 可选字符串参数：去空白、空串与非字符串都当作没传。
#[test]
fn optional_string_arg_trims_and_rejects_blank_values() {
    assert_eq!(
        super::super::optional_string_arg(&json!({ "agent_id": "  subagent-1  " }), "agent_id"),
        Some("subagent-1".to_string())
    );
    assert_eq!(
        super::super::optional_string_arg(&json!({}), "agent_id"),
        None
    );
    assert_eq!(
        super::super::optional_string_arg(&json!({ "agent_id": "   " }), "agent_id"),
        None
    );
    assert_eq!(
        super::super::optional_string_arg(&json!({ "agent_id": 42 }), "agent_id"),
        None
    );
}

/// 作用域参数只接受声明过的取值，大小写不敏感。
#[test]
fn scope_arg_accepts_only_declared_values() {
    assert_eq!(
        super::super::scope_arg(&json!({}), &["self", "all"], "self").unwrap(),
        "self"
    );
    assert_eq!(
        super::super::scope_arg(
            &json!({ "scope": " WORKSPACE " }),
            &["self", "workspace"],
            "self"
        )
        .unwrap(),
        "workspace"
    );
    let error = super::super::scope_arg(&json!({ "scope": "galaxy" }), &["self", "all"], "self")
        .unwrap_err();
    assert!(error.to_string().contains("unsupported scope: galaxy"));
}
