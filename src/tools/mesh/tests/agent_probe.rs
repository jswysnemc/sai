use super::*;

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
        assert_eq!(agents[0]["session_online"], true);
    })
    .await;
}

/// owner 作用域带上本进程打开的其它会话，all 作用域还能带上未打开的会话。
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
        // 第二个会话登记为本进程打开；无主会话不登记在线实例
        let _holder =
            SessionPresenceGuard::register(&mine_dir, &mine.id, SessionOwner::Web).unwrap();
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
            "本进程打开的其它会话也要列出"
        );
        assert!(
            !owner_ids.contains("subagent-orphan"),
            "未打开的会话不属于 owner 作用域"
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
        assert_eq!(orphan_agent["session_online"], false);
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
