use super::*;
use crate::config::AppConfig;
use crate::state::{create_session_for_workspace, StateStore};
use serde_json::{json, Value};

/// 【网格通信】【测试会话】创建与 TUI 相同的会话工具及 Agent，不连接外部模型。
/// 参数: paths 为隔离路径，title 为会话标题
/// 返回: 绑定独立会话的 Agent
fn session_agent(paths: &SaiPaths, title: &str) -> Agent {
    let mut config = AppConfig::default();
    config.memory.enabled = false;
    config.skills.enabled = false;
    config.load_instruction_files = false;
    config.mesh.cross_session = true;
    let state = new_state(paths, title);
    let tools = crate::cli::build_repl_tool_registry_without_mcp_for_session(
        &config,
        paths,
        AgentMode::Yolo,
        state.session_id(),
        state.state_dir(),
    )
    .unwrap();
    let client = OpenAiCompatibleClient::from_config(&config, paths).unwrap();
    Agent::new(config, paths, state, client, tools, AgentMode::Yolo).unwrap()
}

/// 【网格通信】【测试状态】模拟新建会话并初始化持久化记录。
/// 参数: paths 为隔离路径，title 为会话标题；返回新状态存储
fn new_state(paths: &SaiPaths, title: &str) -> StateStore {
    let workspace = crate::runtime_cwd::current_dir().unwrap();
    let session = create_session_for_workspace(paths, &workspace, Some(title)).unwrap();
    let state = StateStore::for_session(paths, &session.id).unwrap();
    state.init_files().unwrap();
    state
}

/// 【网格通信】【测试调用】通过真实工具执行消息投递。
/// 参数: agent 为发送方，args 为工具参数；返回工具投递结果
async fn send(agent: &Agent, args: Value) -> Value {
    let result = agent
        .tools
        .call("mesh_send", &args.to_string())
        .await
        .unwrap();
    serde_json::from_str(&result).unwrap()
}

/// 【网格通信】【双向回归】双方新建会话后，按回复地址返回的结果必须唤醒当前发送方。
/// 参数: 无；返回无；回复落入旧信箱时断言失败
#[tokio::test]
async fn mesh_reply_after_session_switch_reaches_current_sender() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    crate::runtime_cwd::scope(root.path().to_path_buf(), async {
        let mut left = session_agent(&paths, "left-old");
        let old_left = left.state().clone();
        let mut right = session_agent(&paths, "right-old");
        left.replace_state(new_state(&paths, "left-current")).unwrap();
        right.replace_state(new_state(&paths, "right-current")).unwrap();
        let request = send(&left, json!({"to":format!("session:{}", right.session_id()), "text":"reply ok"})).await;
        let inbound = crate::tools::mesh::pending_messages(right.state().state_dir(), right.session_id());
        assert_eq!(inbound.len(), 1);
        send(&right, json!({"to":inbound[0].reply_to, "text":"ok", "correlation_id":request["correlation_id"]})).await;
        let reply = crate::tools::mesh::pending_messages(left.state().state_dir(), left.session_id());
        assert_eq!(reply.len(), 1, "reply was delivered to an old session instead of the current sender");
        assert_eq!(reply[0].text, "ok");
        assert_eq!(reply[0].from, format!("session:{}",right.session_id()));
        assert!(crate::tools::mesh::pending_messages(old_left.state_dir(),old_left.session_id()).is_empty());
        let wake = tokio::time::timeout(std::time::Duration::from_secs(2), left.external_event_monitor().wait_for_wake()).await.unwrap().unwrap().unwrap();
        let ExternalEventWake::Completion(batch) = wake else { panic!("mesh reply should wake the session") };
        assert!(batch.display().contains("ok"));
        left.acknowledge_external_events(&batch).unwrap();
        assert!(crate::tools::mesh::pending_messages(left.state().state_dir(), left.session_id()).is_empty());
    }).await;
}

/// 【网格通信】【迟到注册表】旧预热、模式切换与配置重载的注册表不能覆盖当前身份。
/// 参数: 无；返回无；任一入口重新使用旧回复地址时断言失败
#[tokio::test]
async fn stale_registry_replacements_keep_current_mesh_identity() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    crate::runtime_cwd::scope(root.path().to_path_buf(), async {
        let mut agent = session_agent(&paths, "old");
        let stale = agent.tools.clone();
        agent.replace_state(new_state(&paths, "current")).unwrap();
        for action in ["warmup", "mode", "reload"] {
            match action {
                "warmup" => agent.replace_tools(stale.clone()),
                "mode" => agent.switch_mode(AgentMode::Yolo, stale.clone()).unwrap(),
                _ => {
                    let config = agent.config.clone();
                    let client = OpenAiCompatibleClient::from_config(&config, &paths).unwrap();
                    agent
                        .reload(config, client, stale.clone(), AgentMode::Yolo)
                        .unwrap();
                }
            }
            let expected = format!("session:{}", agent.session_id());
            let result = send(&agent, json!({"to":expected, "text":action})).await;
            assert_eq!(result["from"], expected, "entry={action}");
            assert_eq!(result["reply_to"], expected, "entry={action}");
        }
        // 1. 新 Agent 接受旧注册表时同样以显式状态为准
        let config = agent.config.clone();
        let client = OpenAiCompatibleClient::from_config(&config, &paths).unwrap();
        let rebuilt = Agent::new(
            config,
            &paths,
            new_state(&paths, "rebuilt"),
            client,
            stale,
            AgentMode::Yolo,
        )
        .unwrap();
        let expected = format!("session:{}", rebuilt.session_id());
        let result = send(&rebuilt, json!({"to":expected, "text":"new agent"})).await;
        assert_eq!(result["from"], expected);
    })
    .await;
}

/// 【网格通信】【范围隔离】身份重绑保留工具白名单，关闭跨会话时只允许新会话自己。
/// 参数: 无；返回无；禁用工具恢复或旧归属继续被放行时断言失败
#[tokio::test]
async fn session_rebinding_preserves_tool_filter_and_cross_session_policy() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    crate::runtime_cwd::scope(root.path().to_path_buf(), async {
        let mut agent = session_agent(&paths, "old");
        let old = agent.session_id().to_string();
        agent.tools = agent.tools.clone_filtered(&["mesh_send", "session_probe"]);
        agent.config.mesh.cross_session = false;
        agent.replace_state(new_state(&paths, "current")).unwrap();
        assert!(!agent.tools.contains("subagent"));
        assert!(!agent.tools.contains("run_command"));
        assert!(!agent.tools.contains("agent_probe"));
        let expected = format!("session:{}", agent.session_id());
        let result = send(&agent, json!({"to":expected, "text":"self"})).await;
        assert_eq!(result["from"], expected);
        assert!(agent
            .tools
            .call(
                "mesh_send",
                &json!({"to":format!("session:{old}"), "text":"must be rejected"}).to_string()
            )
            .await
            .is_err());
        let probe: Value = serde_json::from_str(
            &agent
                .tools
                .call("session_probe", r#"{"scope":"self"}"#)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(probe["sessions"][0]["id"], agent.session_id());
    })
    .await;
}

/// 【网格通信】【审计归属】重绑后审计写入新会话，权限热切换句柄继续共享。
/// 参数: 无；返回无；审计仍写入旧会话时断言失败
#[tokio::test]
async fn session_rebinding_moves_permission_audit_to_current_session() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    crate::runtime_cwd::scope(root.path().to_path_buf(), async {
        let mut agent = session_agent(&paths, "old");
        let old_dir = agent.state().state_dir().to_path_buf();
        let profile = crate::permission::PermissionProfile::new(
            crate::permission::PermissionProfileMode::Audited,
            root.path().to_path_buf(),
            Some(crate::permission::PermissionAuditLog::new(
                old_dir.join("permission-audit.jsonl"),
                agent.session_id(),
            )),
        );
        let mode = profile.mode_handle();
        agent.tools.set_permission_profile(profile);
        agent.replace_state(new_state(&paths, "current")).unwrap();
        assert!(std::sync::Arc::ptr_eq(
            &mode,
            &agent.tools.permission_mode_handle().unwrap()
        ));
        agent
            .tools
            .call("session_probe", r#"{"scope":"self"}"#)
            .await
            .unwrap();
        let audit =
            std::fs::read_to_string(agent.state().state_dir().join("permission-audit.jsonl"))
                .unwrap();
        assert!(audit.contains(agent.session_id()));
        assert!(!old_dir.join("permission-audit.jsonl").exists());
    })
    .await;
}

/// 【会话工具】【子任务归属】切换后仅能管理当前会话的子任务，恢复旧会话时恢复其原有列表。
/// 参数: 无；返回无；列表仍绑定旧会话时断言失败
#[tokio::test]
async fn session_rebinding_tracks_subagent_owner_on_new_and_resume() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    crate::runtime_cwd::scope(root.path().to_path_buf(), async {
        let mut agent = session_agent(&paths, "old");
        let old = agent.state().clone();
        let (old_child, _old_cancel) = crate::tools::subagent_state::create_subagent_for_owner(
            &old.state_dir().to_string_lossy(),
            "old child".into(),
            "general".into(),
            1,
        );
        agent.replace_state(new_state(&paths, "current")).unwrap();
        let (current_child, _current_cancel) =
            crate::tools::subagent_state::create_subagent_for_owner(
                &agent.state().state_dir().to_string_lossy(),
                "current child".into(),
                "general".into(),
                1,
            );
        for (state, expected) in [
            (agent.state().clone(), current_child.id),
            (old, old_child.id),
        ] {
            agent.replace_state(state).unwrap();
            let result: Value = serde_json::from_str(
                &agent
                    .tools
                    .call("subagent", r#"{"action":"list"}"#)
                    .await
                    .unwrap(),
            )
            .unwrap();
            let children = result["subagents"].as_array().unwrap();
            assert_eq!(children.len(), 1);
            assert_eq!(children[0]["id"], expected);
            let address = format!("session:{}", agent.session_id());
            let sent = send(&agent, json!({"to":address,"text":"current identity"})).await;
            assert_eq!(sent["reply_to"], address);
        }
    })
    .await;
}
