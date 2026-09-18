use super::*;

/// session_probe 的 self 作用域报出在线实例与轮次状态。
#[tokio::test]
async fn session_probe_reports_nonexclusive_instances_and_idle_turn() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace-a");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (session, state_dir) = session_in(&paths, &cwd, "probe");
        let _presence =
            SessionPresenceGuard::register(&state_dir, &session.id, SessionOwner::Repl).unwrap();
        let _web =
            SessionPresenceGuard::register(&state_dir, &session.id, SessionOwner::Web).unwrap();

        let output = probe(
            &registry_for(&paths, &session),
            "session_probe",
            r#"{"scope":"self"}"#,
        )
        .await;
        let sessions = output["sessions"].as_array().unwrap();

        assert_eq!(output["scope"], "self");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0]["id"], session.id);
        assert_eq!(sessions[0]["title"], "probe");
        assert_eq!(sessions[0]["is_workspace_current"], true);
        assert_eq!(sessions[0]["is_self"], true);
        assert_eq!(sessions[0]["online"], true);
        assert_eq!(sessions[0]["idle"], true);
        assert_eq!(sessions[0]["running"], false);
        assert_eq!(sessions[0]["instances"][0]["owner"], "repl");
        assert_eq!(sessions[0]["instances"][0]["pid"], std::process::id());
        assert_eq!(sessions[0]["instances"].as_array().unwrap().len(), 2);
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
            "没有在线实例的会话不应出现在探测结果里: {hidden}"
        );
        assert!(
            hidden["omitted_inactive"].as_u64().unwrap() >= 1,
            "应统计被省略的非活动会话: {hidden}"
        );

        let _holder =
            SessionPresenceGuard::register(&foreign_dir, &foreign.id, SessionOwner::Repl).unwrap();
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
        // 没有在线实例的会话不进探测结果，给它挂一个才看得到
        let _holder =
            SessionPresenceGuard::register(&newer_dir, &newer.id, SessionOwner::Repl).unwrap();
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
        super::super::session_probe::DESCRIPTION_EN,
        super::super::session_probe::DESCRIPTION_ZH,
    ] {
        assert!(
            description.contains("self.session_id"),
            "工具说明必须点名 self.session_id: {description}"
        );
        assert!(
            description.contains("never in it") || description.contains("绝不会出现在里面"),
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

/// 同一工作区里没有在线实例的会话不出现在探测结果中；当前会话本身仍展示。
#[tokio::test]
async fn session_probe_omits_unopened_sessions_in_the_same_workspace() {
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

/// 【网格】【会话探测】另一工作区刚打开、还没发过提示词的会话，只要实例在线就应被发现。
#[tokio::test]
async fn session_probe_finds_an_empty_open_session_in_another_workspace() {
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
            SessionPresenceGuard::register(&foreign_dir, &foreign.id, SessionOwner::Repl).unwrap();

        let output = probe(&registry_for(&paths, &local), "session_probe", "{}").await;
        assert_eq!(output["scope"], "all");
        let sessions = output["sessions"].as_array().unwrap();
        let found = sessions.iter().find(|session| session["id"] == foreign.id);
        let found = found.unwrap_or_else(|| panic!("empty held session missing: {output}"));
        assert_eq!(found["online"], true);
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

        assert!(ids.is_empty(), "自己不在 sessions 数组里: {output}");
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
