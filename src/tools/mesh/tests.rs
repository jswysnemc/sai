use crate::paths::SaiPaths;
use crate::runner::{ActiveRunGuard, SessionHolderGuard, SessionOwner};
use crate::state::{create_session_for_workspace, locate_session_dirs, SessionInfo};
use crate::tools::subagent_persistence::{self, PersistedSubagent};
use crate::tools::subagent_state::{create_subagent_for_owner, SubagentSnapshot};
use crate::tools::ToolRegistry;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// 构造绑定当前会话的探测工具注册表。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `session`: 当前会话
///
/// 返回:
/// - 已注册两个探测工具的注册表
fn registry_for(paths: &SaiPaths, session: &SessionInfo) -> ToolRegistry {
    let state_dir = locate_session_dirs(paths, &session.id).unwrap().1;
    let mut registry = ToolRegistry::new();
    super::register(
        &mut registry,
        paths.clone(),
        state_dir.display().to_string(),
        session.id.clone(),
        false,
    );
    registry
}

/// 调用探测工具并把输出解析为 JSON。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `tool`: 工具名
/// - `arguments`: 原始 JSON 参数
///
/// 返回:
/// - 解析后的工具输出
async fn probe(registry: &ToolRegistry, tool: &str, arguments: &str) -> Value {
    let output = registry.call(tool, arguments).await.unwrap();
    serde_json::from_str(&output).unwrap()
}

/// 在当前工作目录里建一个会话并返回其状态目录。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `workspace`: 工作区目录
/// - `title`: 会话标题
///
/// 返回:
/// - (会话信息, 会话状态目录)
fn session_in(paths: &SaiPaths, workspace: &Path, title: &str) -> (SessionInfo, PathBuf) {
    let session = create_session_for_workspace(paths, workspace, Some(title)).unwrap();
    let state_dir = locate_session_dirs(paths, &session.id).unwrap().1;
    (session, state_dir)
}

/// 在指定会话状态目录里落一份已持久化的子智能体记录。
///
/// 参数:
/// - `state_dir`: 会话状态目录
/// - `id`: 子智能体 ID
///
/// 返回:
/// - 无
fn persist_subagent(state_dir: &Path, id: &str) {
    let owner_key = state_dir.display().to_string();
    subagent_persistence::save(
        &owner_key,
        std::slice::from_ref(&PersistedSubagent {
            owner_key: owner_key.clone(),
            snapshot: SubagentSnapshot {
                id: id.to_string(),
                goal_id: None,
                description: "persisted".to_string(),
                subagent_type: "explore".to_string(),
                status: "completed".to_string(),
                max_steps: 3,
                started_at: 1,
                updated_at: 2,
                step: 2,
                phase: None,
                last_tool: Some("grep".to_string()),
                result: Some("done".to_string()),
                error: None,
                stats: Some(json!({ "total_tokens": 4096 })),
                worktree_root: None,
                worktree_branch: None,
                parent_workdir: None,
                worktree_merge: None,
                persistent: false,
                pending_messages: 0,
                turns_completed: 0,
            },
            timeline: Vec::new(),
            finish_notified: true,
        }),
    )
    .unwrap();
}

/// session_probe 的 self 作用域报出持有者、观察者数与轮次状态。
#[tokio::test]
async fn session_probe_reports_holder_watchers_and_idle_turn() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace-a");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (session, state_dir) = session_in(&paths, &cwd, "probe");
        let holder =
            SessionHolderGuard::acquire(&state_dir, &session.id, SessionOwner::Repl).unwrap();
        holder.set_watchers(3).unwrap();

        let output = probe(&registry_for(&paths, &session), "session_probe", r#"{"scope":"self"}"#).await;
        let sessions = output["sessions"].as_array().unwrap();

        assert_eq!(output["scope"], "self");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0]["id"], session.id);
        assert_eq!(sessions[0]["title"], "probe");
        assert_eq!(sessions[0]["is_workspace_current"], true);
        assert_eq!(sessions[0]["is_self"], true);
        assert_eq!(sessions[0]["held"], true);
        assert_eq!(sessions[0]["idle"], true);
        assert_eq!(sessions[0]["running"], false);
        assert_eq!(sessions[0]["holder"]["owner"], "repl");
        assert_eq!(sessions[0]["holder"]["pid"], std::process::id());
        assert_eq!(sessions[0]["holder"]["alive"], true);
        assert_eq!(sessions[0]["holder"]["watchers"], 3);
    })
    .await;
}

/// 正在跑一轮的会话会被标出持有轮次的 owner。
#[tokio::test]
async fn session_probe_marks_the_session_that_is_running_a_turn() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace-a");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (session, state_dir) = session_in(&paths, &cwd, "running");
        let registry = registry_for(&paths, &session);

        let guard =
            ActiveRunGuard::acquire_with_state_dir(&session.id, SessionOwner::Web, &state_dir)
                .unwrap();
        // 自己不在 sessions 里,用 self 作用域检查本会话的运行状态
        let running = probe(&registry, "session_probe", r#"{"scope":"self"}"#).await;
        assert_eq!(running["sessions"][0]["running"], true);
        assert_eq!(running["sessions"][0]["active_run"]["owner"], "web");

        // 崩溃残留的锁会让会话永远显示"正在跑一轮"，释放后必须回到空闲
        drop(guard);
        let idle = probe(&registry, "session_probe", r#"{"scope":"self"}"#).await;
        assert_eq!(idle["sessions"][0]["running"], false);
        assert!(idle["sessions"][0]["active_run"].is_null());
    })
    .await;
}

/// workspace 作用域只报本工作区的活动会话，all 作用域跨工作区但仍隐藏非活动会话。
#[tokio::test]
async fn session_probe_scopes_separate_workspace_from_all() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace-a");
    let other = temp.path().join("workspace-b");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::create_dir_all(&other).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (local, _) = session_in(&paths, &cwd, "local");
        let (foreign, foreign_dir) = session_in(&paths, &other, "foreign");
        let registry = registry_for(&paths, &local);

        let in_workspace = probe(&registry, "session_probe", r#"{"scope":"workspace"}"#).await;
        let ids = in_workspace["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|session| session["id"].as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert!(
            !ids.contains(&local.id),
            "sessions 数组只列其它会话,自己由顶层 self 标识: {in_workspace}"
        );
        assert!(
            !ids.contains(&foreign.id),
            "workspace 作用域不该带上别的工作区的会话"
        );

        let hidden = probe(&registry, "session_probe", r#"{"scope":"all"}"#).await;
        let hidden_ids = hidden["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|session| session["id"].as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(
            !hidden_ids.contains(&foreign.id),
            "没有存活持有者的会话不应出现在探测结果里: {hidden}"
        );
        assert!(
            hidden["omitted_inactive"].as_u64().unwrap() >= 1,
            "应统计被省略的非活动会话: {hidden}"
        );

        let _holder =
            SessionHolderGuard::acquire(&foreign_dir, &foreign.id, SessionOwner::Repl).unwrap();
        let everything = probe(&registry, "session_probe", r#"{"scope":"all"}"#).await;
        let all_ids = everything["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|session| session["id"].as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(!all_ids.contains(&local.id), "自己不在 sessions 里");
        assert!(all_ids.contains(&foreign.id));
        // 共享指针条目:foreign 是自己工作区的 current;local 的指针指向自己但被排除
        let current = everything["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|session| session["is_workspace_current"] == true)
            .count();
        assert_eq!(current, 1, "列表里只有 foreign 的指针条目");
        assert_eq!(everything["self"]["session_id"], local.id);
    })
    .await;
}

/// 判定「哪个是我」只能看 is_self：共享指针常常属于别的终端。
#[tokio::test]
async fn session_probe_self_identity_never_depends_on_the_workspace_pointer() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace-a");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        // 同一工作区里后建的会话抢走共享指针，但「我」仍然是先建的那个
        let (local, _) = session_in(&paths, &cwd, "local");
        let (newer, newer_dir) = session_in(&paths, &cwd, "newer");
        // 没有存活持有者的会话不进探测结果，给它挂一个才看得到
        let _holder =
            SessionHolderGuard::acquire(&newer_dir, &newer.id, SessionOwner::Repl).unwrap();
        let registry = registry_for(&paths, &local);

        let output = probe(&registry, "session_probe", r#"{"scope":"workspace"}"#).await;
        let sessions = output["sessions"].as_array().unwrap();
        // 自己不在 sessions 里;后建的会话持有共享指针,仍在列表中
        assert!(
            sessions.iter().all(|session| session["is_self"] == false),
            "sessions 里不应出现自己: {output}"
        );
        assert_eq!(output["self"]["session_id"], local.id);
        // 共享指针落在后建的会话上,证明它不能当身份判据
        let pointer = sessions
            .iter()
            .find(|session| session["is_workspace_current"] == true)
            .expect("工作区共享指针应指向某条会话");
        assert_eq!(pointer["id"], newer.id);
        assert_eq!(pointer["is_self"], false);
    })
    .await;
}

/// 工具说明必须声明 sessions 数组不含自己,且不能把 is_workspace_current 说成身份判据。
///
/// 两种语言都要覆盖：模型在中文界面下读到的是中文那份说明。
#[test]
fn probe_description_documents_is_self() {
    for description in [
        super::session_probe::DESCRIPTION_EN,
        super::session_probe::DESCRIPTION_ZH,
    ] {
        assert!(
            description.contains("self.session_id"),
            "工具说明必须点名 self.session_id: {description}"
        );
        assert!(
            description.contains("never in it")
                || description.contains("绝不会出现在里面"),
            "工具说明必须声明 sessions 数组不含自己: {description}"
        );
        assert!(
            !description.contains("whether it is its workspace's current session")
                && !description.contains("是否为所在工作区的当前会话"),
            "工具说明不应再把 is_workspace_current 说成身份判据: {description}"
        );
        assert!(
            description.contains("Never use is_workspace_current")
                || description.contains("绝不要用 is_workspace_current"),
            "工具说明必须明确排除用 is_workspace_current 判身份: {description}"
        );
    }
}

/// 同一工作区里没有存活持有者的会话不出现在探测结果中；当前会话本身仍展示。
#[tokio::test]
async fn session_probe_omits_unheld_sessions_in_the_same_workspace() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace-a");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (local, _) = session_in(&paths, &cwd, "local");
        let (idle, _) = session_in(&paths, &cwd, "idle");
        let registry = registry_for(&paths, &local);

        let output = probe(&registry, "session_probe", r#"{"scope":"workspace"}"#).await;
        let ids = output["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|session| session["id"].as_str().unwrap().to_string())
            .collect::<Vec<_>>();

        assert!(
            !ids.contains(&local.id),
            "自己不在 sessions 数组里,由顶层 self 标识: {output}"
        );
        assert!(
            !ids.contains(&idle.id),
            "非活动会话不应出现在探测结果里: {output}"
        );
        assert!(
            output["omitted_inactive"].as_u64().unwrap() >= 1,
            "应统计被省略的非活动会话: {output}"
        );
    })
    .await;
}

/// 【网格】【会话探测】另一工作区刚打开、还没发过提示词的会话，只要持有者存活就应被发现。
#[tokio::test]
async fn session_probe_finds_an_empty_held_session_in_another_workspace() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace-a");
    let other = temp.path().join("workspace-b");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::create_dir_all(&other).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (local, _) = session_in(&paths, &cwd, "local");
        let foreign_sessions = crate::state::list_sessions_for_workspace(&paths, &other).unwrap();
        let foreign = foreign_sessions
            .iter()
            .find(|session| session.id == "default")
            .cloned()
            .expect("opening a workspace creates the default session before any prompt");
        let foreign_dir =
            crate::state::state_dir_for_workspace_session(&paths, &other, &foreign.id)
                .unwrap()
                .1;
        let _holder =
            SessionHolderGuard::acquire(&foreign_dir, &foreign.id, SessionOwner::Repl).unwrap();

        let output = probe(&registry_for(&paths, &local), "session_probe", "{}").await;
        assert_eq!(output["scope"], "all");
        let sessions = output["sessions"].as_array().unwrap();
        let found = sessions.iter().find(|session| session["id"] == foreign.id);
        let found = found.unwrap_or_else(|| panic!("empty held session missing: {output}"));
        assert_eq!(found["held"], true);
        assert_eq!(found["idle"], true);
        assert_eq!(found["running"], false);
        assert_eq!(found["is_self"], false);
    })
    .await;
}

/// workspace 作用域按规范化后的当前目录计算工作区 ID。
///
/// 会话目录名由 `workspace_scope_for_path` 规范化路径后哈希得出，查询侧若直接
/// 哈希原始 cwd 就会算出另一个 ID。Windows 上原始 cwd 可能是 8.3 短名
/// （`RUNNER~1`）、大小写不一致或带 `\\?\` 前缀；Linux 上的等价场景是 cwd 里
/// 含符号链接，这条用例因此在 Linux 上就能复现 Windows 的失败。
#[cfg(unix)]
#[tokio::test]
async fn session_probe_workspace_scope_uses_the_canonicalized_cwd() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let real = temp.path().join("real-workspace");
    let alias = temp.path().join("alias-workspace");
    std::fs::create_dir_all(&real).unwrap();
    std::os::unix::fs::symlink(&real, &alias).unwrap();

    crate::runtime_cwd::scope(alias, async {
        let (local, _) = session_in(&paths, &real, "local");
        let registry = registry_for(&paths, &local);

        let output = probe(&registry, "session_probe", r#"{"scope":"workspace"}"#).await;
        let ids = output["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|session| session["id"].as_str().unwrap().to_string())
            .collect::<Vec<_>>();

        assert!(
            ids.is_empty(),
            "自己不在 sessions 数组里: {output}"
        );
        assert_eq!(output["self"]["session_id"], local.id);
    })
    .await;
}

/// 非法作用域直接报错，而不是静默退化成默认作用域。
#[tokio::test]
async fn session_probe_rejects_an_unknown_scope() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace-a");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (session, _) = session_in(&paths, &cwd, "probe");
        let registry = registry_for(&paths, &session);

        assert!(registry
            .call("session_probe", r#"{"scope":"galaxy"}"#)
            .await
            .is_err());
    })
    .await;
}

/// agent_probe 的 self 作用域列出当前会话在跑的子智能体。
#[tokio::test]
async fn agent_probe_lists_subagents_of_the_current_session() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace-a");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (session, state_dir) = session_in(&paths, &cwd, "owner");
        let owner_key = state_dir.display().to_string();
        let (snapshot, _cancel) =
            create_subagent_for_owner(&owner_key, "inspect".to_string(), "general".to_string(), 5);

        let output = probe(&registry_for(&paths, &session), "agent_probe", "{}").await;
        let agents = output["agents"].as_array().unwrap();

        assert_eq!(output["scope"], "self");
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0]["agent_id"], snapshot.id);
        assert_eq!(agents[0]["session_id"], session.id);
        assert_eq!(agents[0]["type"], "general");
        assert_eq!(agents[0]["status"], "running");
        assert_eq!(agents[0]["step"], 0);
        assert_eq!(agents[0]["max_steps"], 5);
        assert_eq!(agents[0]["holder_alive"], true);
    })
    .await;
}

/// owner 作用域带上本进程持有的其它会话，all 作用域还能带上无人持有的会话。
#[tokio::test]
async fn agent_probe_owner_scope_covers_every_session_of_this_process() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace-a");
    let other = temp.path().join("workspace-b");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::create_dir_all(&other).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (local, local_dir) = session_in(&paths, &cwd, "owner");
        let (mine, mine_dir) = session_in(&paths, &other, "second-session");
        let (_, orphan_dir) = session_in(&paths, &other, "unheld-session");
        persist_subagent(&local_dir, "subagent-local");
        persist_subagent(&mine_dir, "subagent-mine");
        persist_subagent(&orphan_dir, "subagent-orphan");
        // 第二个会话登记为本进程持有；无主会话不登记持有者
        let _holder = SessionHolderGuard::acquire(&mine_dir, &mine.id, SessionOwner::Web).unwrap();
        let registry = registry_for(&paths, &local);

        let owner = probe(&registry, "agent_probe", r#"{"scope":"owner"}"#).await;
        let owner_ids = owner["agents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|agent| agent["agent_id"].as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(owner_ids.contains("subagent-local"));
        assert!(
            owner_ids.contains("subagent-mine"),
            "本进程持有的其它会话也要列出"
        );
        assert!(
            !owner_ids.contains("subagent-orphan"),
            "无人持有的会话不属于 owner 作用域"
        );

        let all = probe(&registry, "agent_probe", r#"{"scope":"all"}"#).await;
        let all_ids = all["agents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|agent| agent["agent_id"].as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(all_ids.contains("subagent-orphan"));
        let orphan_agent = all["agents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|agent| agent["agent_id"] == "subagent-orphan")
            .unwrap();
        assert_eq!(orphan_agent["holder_alive"], false);
        assert_eq!(orphan_agent["status"], "completed");
        assert_eq!(orphan_agent["last_tool"], "grep");
        assert_eq!(orphan_agent["total_tokens"], 4096);
    })
    .await;
}

/// 按 agent_id 精确查一个子智能体，查不到时明确回报未命中。
#[tokio::test]
async fn agent_probe_filters_by_agent_id() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace-a");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (session, state_dir) = session_in(&paths, &cwd, "owner");
        let owner_key = state_dir.display().to_string();
        let (snapshot, _cancel) =
            create_subagent_for_owner(&owner_key, "inspect".to_string(), "general".to_string(), 5);
        let registry = registry_for(&paths, &session);

        let hit = probe(
            &registry,
            "agent_probe",
            &format!(r#"{{"agent_id":"{}"}}"#, snapshot.id),
        )
        .await;
        assert_eq!(hit["count"], 1);
        assert_eq!(hit["agents"][0]["agent_id"], snapshot.id);

        let miss = probe(&registry, "agent_probe", r#"{"agent_id":"subagent-nope"}"#).await;
        assert_eq!(miss["ok"], false);
        assert!(miss["agents"].as_array().unwrap().is_empty());
    })
    .await;
}

/// 交互式会话注册表里能直接拿到两个探测工具。
#[tokio::test]
async fn interactive_registry_exposes_both_probe_tools() {
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
    })
    .await;
}

/// 可选字符串参数：去空白、空串与非字符串都当作没传。
#[test]
fn optional_string_arg_trims_and_rejects_blank_values() {
    assert_eq!(
        super::optional_string_arg(&json!({ "agent_id": "  subagent-1  " }), "agent_id"),
        Some("subagent-1".to_string())
    );
    assert_eq!(super::optional_string_arg(&json!({}), "agent_id"), None);
    assert_eq!(
        super::optional_string_arg(&json!({ "agent_id": "   " }), "agent_id"),
        None
    );
    assert_eq!(
        super::optional_string_arg(&json!({ "agent_id": 42 }), "agent_id"),
        None
    );
}

/// 作用域参数只接受声明过的取值，大小写不敏感。
#[test]
fn scope_arg_accepts_only_declared_values() {
    assert_eq!(
        super::scope_arg(&json!({}), &["self", "all"], "self").unwrap(),
        "self"
    );
    assert_eq!(
        super::scope_arg(
            &json!({ "scope": " WORKSPACE " }),
            &["self", "workspace"],
            "self"
        )
        .unwrap(),
        "workspace"
    );
    let error =
        super::scope_arg(&json!({ "scope": "galaxy" }), &["self", "all"], "self").unwrap_err();
    assert!(error.to_string().contains("unsupported scope: galaxy"));
}

/// 构造可切换跨会话开关的工具注册表。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `session`: 当前会话
/// - `cross_session`: 是否允许投递到当前会话之外
///
/// 返回:
/// - 已注册全部网格工具的注册表
fn registry_with_cross_session(
    paths: &SaiPaths,
    session: &SessionInfo,
    cross_session: bool,
) -> ToolRegistry {
    let state_dir = locate_session_dirs(paths, &session.id).unwrap().1;
    let mut registry = ToolRegistry::new();
    super::register(
        &mut registry,
        paths.clone(),
        state_dir.display().to_string(),
        session.id.clone(),
        cross_session,
    );
    registry
}

/// 归属隔离的默认行为：投给别的会话必须被拒绝。
///
/// 这是网格工具的安全底线——默认开放等于任何 agent 都能往任意会话注入消息。
#[tokio::test]
async fn send_outside_the_session_is_rejected_by_default() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (local, _) = session_in(&paths, &cwd, "local");
        let (other, _) = session_in(&paths, &cwd, "other");

        let error = registry_with_cross_session(&paths, &local, false)
            .call(
                "mesh_send",
                &format!(r#"{{"to":"session:{}","text":"hi"}}"#, other.id),
            )
            .await
            .unwrap_err();

        assert!(
            error.to_string().contains("outside this session"),
            "cross-session send must be rejected by default: {error}"
        );
    })
    .await;
}

/// 显式开启 `mesh.cross_session` 后允许投递到别的会话。
#[tokio::test]
async fn send_outside_the_session_is_allowed_when_cross_session_is_enabled() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (local, _) = session_in(&paths, &cwd, "local");
        let (other, _) = session_in(&paths, &cwd, "other");

        let output = registry_with_cross_session(&paths, &local, true)
            .call(
                "mesh_send",
                &format!(r#"{{"to":"session:{}","text":"hi"}}"#, other.id),
            )
            .await
            .unwrap();
        let parsed: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["ok"], true, "{parsed}");
        assert!(parsed["correlation_id"].as_str().is_some(), "{parsed}");
    })
    .await;
}

/// 白名单 Agent（如"代码 Agent"的 enabled_tools）过滤注册表后，
/// `mesh.cross_session` 开关必须仍然生效：过滤若把会话归属与开关重置，
/// 即使配置了 true 也会被权限策略拦下。
#[tokio::test]
async fn cross_session_survives_the_agent_whitelist_filter() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (local, _) = session_in(&paths, &cwd, "local");
        let (other, _) = session_in(&paths, &cwd, "other");
        let state_dir = locate_session_dirs(&paths, &local.id).unwrap().1;

        // 白名单配置等价于"代码 Agent"的 enabled_tools，并显式开启跨会话
        let mut config = crate::config::AppConfig::default();
        config.mesh.cross_session = true;
        config.agent_runtime = Some(crate::config::AgentRuntimeOverride {
            enabled_tools: vec!["mesh_send".to_string()],
            exclusive: false,
            deferred_tools: Vec::new(),
            skills_full: Vec::new(),
            skills_named: Vec::new(),
        });

        // 与真实路径一致：先注册（含会话归属），再过白名单过滤，最后绑定权限配置
        let mut registry = ToolRegistry::new();
        super::register(
            &mut registry,
            paths.clone(),
            state_dir.display().to_string(),
            local.id.clone(),
            config.mesh.cross_session,
        );
        registry.set_session_ownership(
            state_dir.display().to_string(),
            local.id.clone(),
            config.mesh.cross_session,
        );
        let mut filtered = crate::runner::submission_tools::apply_enabled_tools_filter(
            registry,
            &config,
            crate::runner::SubmissionSource::Repl,
        )
        .unwrap();
        filtered.set_permission_profile(crate::permission::PermissionProfile::new(
            crate::permission::PermissionProfileMode::Yolo,
            cwd.clone(),
            None,
        ));

        let output = filtered
            .call(
                "mesh_send",
                &format!(r#"{{"to":"session:{}","text":"hi"}}"#, other.id),
            )
            .await
            .unwrap();
        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["ok"], true, "{parsed}");
    })
    .await;
}

/// 会话内投递：mesh_send 之后进入会话队列待投递，确认后不再重复注入。
#[tokio::test]
async fn send_queues_an_active_receipt_without_recv() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (session, state_dir) = session_in(&paths, &cwd, "local");
        let registry = registry_with_cross_session(&paths, &session, false);
        assert!(
            !registry.contains("mesh_recv"),
            "mesh_recv must not be registered"
        );

        let sent: Value = serde_json::from_str(
            &registry
                .call(
                    "mesh_send",
                    &format!(r#"{{"to":"session:{}","text":"ping"}}"#, session.id),
                )
                .await
                .unwrap(),
        )
        .unwrap();
        let correlation_id = sent["correlation_id"].as_str().unwrap().to_string();

        let pending = super::next_pending(&state_dir, &session.id)
            .expect("sent message should be queued as an active receipt");
        assert_eq!(
            pending.correlation_id.as_deref(),
            Some(correlation_id.as_str())
        );
        assert_eq!(pending.text, "ping");

        super::acknowledge_mesh_messages(&state_dir, std::slice::from_ref(&pending.id));
        assert!(
            super::next_pending(&state_dir, &session.id).is_none(),
            "acknowledged mesh messages must not be re-delivered"
        );
    })
    .await;
}

/// 投递成功后立即返回，不再阻塞等待回复。
#[tokio::test]
async fn mesh_send_returns_without_waiting_for_a_reply() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (session, _) = session_in(&paths, &cwd, "local");

        let output = registry_with_cross_session(&paths, &session, false)
            .call(
                "mesh_send",
                &format!(
                    r#"{{"to":"session:{}","text":"ping"}}"#,
                    session.id
                ),
            )
            .await
            .unwrap();
        let parsed: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["ok"], true, "{parsed}");
        assert!(parsed["correlation_id"].as_str().is_some());
        assert!(parsed.get("timed_out").is_none());
        assert!(parsed.get("reply").is_none());
        assert!(parsed.get("expect_reply").is_none());
    })
    .await;
}
