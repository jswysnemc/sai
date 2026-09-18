use super::*;

/// session_create 只登记新会话：当前会话保持不变，新会话进入索引。
#[tokio::test]
async fn session_create_registers_a_session_without_switching_current() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (session, _) = session_in(&paths, &cwd, "current");
        let registry = registry_for(&paths, &session);

        let created: Value =
            serde_json::from_str(&registry.call("session_create", "{}").await.unwrap()).unwrap();

        assert_eq!(created["ok"], true, "{created}");
        assert_eq!(created["activated"], false, "{created}");
        assert_eq!(created["title"], "New session", "{created}");
        let new_id = created["session_id"].as_str().unwrap().to_string();
        assert_ne!(new_id, session.id);
        assert_eq!(created["address"], format!("session:{new_id}"));

        // 创建不激活：当前会话仍是原来的
        assert_eq!(crate::state::active_session(&paths).unwrap().id, session.id);
        // 新会话已登记进索引
        assert!(crate::state::list_all_sessions(&paths)
            .unwrap()
            .iter()
            .any(|located| located.info.id == new_id));
    })
    .await;
}

/// 显式传 title 时会话获得该名称；省略时留给首条消息自动命名。
#[tokio::test]
async fn session_create_with_explicit_title_names_the_session() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (session, _) = session_in(&paths, &cwd, "current");
        let registry = registry_for(&paths, &session);

        let created: Value = serde_json::from_str(
            &registry
                .call("session_create", r#"{"title":"researcher"}"#)
                .await
                .unwrap(),
        )
        .unwrap();

        assert_eq!(created["title"], "researcher", "{created}");
        let new_id = created["session_id"].as_str().unwrap();
        let listed = crate::state::list_all_sessions(&paths)
            .unwrap()
            .into_iter()
            .find(|located| located.info.id == new_id)
            .expect("created session must be listed");
        assert_eq!(listed.info.title, "researcher");
    })
    .await;
}

/// 创建的会话立即可按 id 收到 mesh_send 投递（ID 键控的会话通信）。
#[tokio::test]
async fn created_session_receives_mesh_send_by_id() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (session, _) = session_in(&paths, &cwd, "current");
        // 新会话不是当前会话，投递属于跨会话，需要显式开启
        let registry = registry_with_cross_session(&paths, &session, true);

        let created: Value =
            serde_json::from_str(&registry.call("session_create", "{}").await.unwrap()).unwrap();
        let new_id = created["session_id"].as_str().unwrap().to_string();

        let sent: Value = serde_json::from_str(
            &registry
                .call(
                    "mesh_send",
                    &format!(r#"{{"to":"session:{new_id}","text":"welcome aboard"}}"#),
                )
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(sent["ok"], true, "{sent}");

        let state_dir = locate_session_dirs(&paths, &new_id).unwrap().1;
        let pending = super::super::next_pending(&state_dir, &new_id)
            .expect("created session must receive the mesh message");
        assert_eq!(pending.text, "welcome aboard");
        assert_eq!(pending.to, format!("session:{new_id}"));
    })
    .await;
}

/// 按 id 激活：当前指针切到目标会话，用户即可 /resume。
#[tokio::test]
async fn session_activate_by_id_switches_the_current_session() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (session, _) = session_in(&paths, &cwd, "current");
        let registry = registry_for(&paths, &session);
        let created: Value =
            serde_json::from_str(&registry.call("session_create", "{}").await.unwrap()).unwrap();
        let new_id = created["session_id"].as_str().unwrap().to_string();

        let activated: Value = serde_json::from_str(
            &registry
                .call(
                    "session_activate",
                    &format!(r#"{{"session_id":"{new_id}"}}"#),
                )
                .await
                .unwrap(),
        )
        .unwrap();

        assert_eq!(activated["ok"], true, "{activated}");
        assert_eq!(activated["session_id"], new_id);
        assert_eq!(activated["address"], format!("session:{new_id}"));
        assert_eq!(
            crate::state::active_session(&paths).unwrap().id,
            new_id,
            "激活后当前指针必须指向新会话"
        );
    })
    .await;
}

/// 按唯一名称激活：只匹配显式命名的会话。
#[tokio::test]
async fn session_activate_by_unique_name_switches_current() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        // current 先建，随后 beta 抢走指针；按名字应切回 current
        let (current, _) = session_in(&paths, &cwd, "alpha");
        let (beta, _) = session_in(&paths, &cwd, "beta");
        assert_ne!(beta.id, current.id);
        let registry = registry_for(&paths, &current);

        let activated: Value = serde_json::from_str(
            &registry
                .call("session_activate", r#"{"name":"alpha"}"#)
                .await
                .unwrap(),
        )
        .unwrap();

        assert_eq!(activated["ok"], true, "{activated}");
        assert_eq!(activated["session_id"], current.id);
        assert_eq!(crate::state::active_session(&paths).unwrap().id, current.id);
    })
    .await;
}

/// 重名时拒绝猜测：报错并列出全部候选 id。
#[tokio::test]
async fn session_activate_rejects_ambiguous_names_with_candidate_ids() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace-a");
    let other = temp.path().join("workspace-b");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::create_dir_all(&other).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (session, _) = session_in(&paths, &cwd, "current");
        let first = create_session_for_workspace(&paths, &cwd, Some("dup")).unwrap();
        let second = create_session_for_workspace(&paths, &other, Some("dup")).unwrap();
        let registry = registry_for(&paths, &session);
        // create_session_for_workspace 会顺带激活，先记下失败调用前的指针
        let pointer_before = crate::state::active_session(&paths).unwrap().id;

        let error = registry
            .call("session_activate", r#"{"name":"dup"}"#)
            .await
            .unwrap_err();

        let message = error.to_string();
        assert!(
            message.contains("ambiguous"),
            "重名必须报 ambiguous 错误: {message}"
        );
        assert!(
            message.contains(&first.id),
            "候选里必须带第一个会话 id: {message}"
        );
        assert!(
            message.contains(&second.id),
            "候选里必须带第二个会话 id: {message}"
        );
        // 拒绝猜测时不能顺手切换当前会话
        assert_eq!(
            crate::state::active_session(&paths).unwrap().id,
            pointer_before
        );
    })
    .await;
}

/// 占位标题（New session / Default）不算显式名称，永不参与名称匹配。
#[tokio::test]
async fn session_activate_never_matches_placeholder_titles() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (session, _) = session_in(&paths, &cwd, "current");
        // 未命名会话（标题 "New session"）与各工作区都有的 "Default" 都存在
        let registry = registry_for(&paths, &session);
        let _ = create_session_for_workspace(&paths, &cwd, None).unwrap();

        for name in ["New session", "Default"] {
            let error = registry
                .call("session_activate", &format!(r#"{{"name":"{name}"}}"#))
                .await
                .unwrap_err();
            assert!(
                error.to_string().contains("no session named"),
                "占位标题 {name} 不应可按名称激活: {error}"
            );
        }
    })
    .await;
}

/// 参数约束：session_id 与 name 二选一，都传或都不传都报错。
#[tokio::test]
async fn session_activate_requires_exactly_one_of_id_or_name() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (session, _) = session_in(&paths, &cwd, "current");
        let registry = registry_for(&paths, &session);

        let neither = registry.call("session_activate", "{}").await.unwrap_err();
        assert!(
            neither.to_string().contains("missing required argument"),
            "{neither}"
        );

        let both = registry
            .call(
                "session_activate",
                &format!(r#"{{"session_id":"{}","name":"current"}}"#, session.id),
            )
            .await
            .unwrap_err();
        assert!(both.to_string().contains("not both"), "{both}");
    })
    .await;
}

/// 跨工作区激活受 mesh.cross_session 约束：默认拒绝，显式开启后切换目标工作区的指针。
#[tokio::test]
async fn session_activate_across_workspaces_requires_cross_session() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace-a");
    let other = temp.path().join("workspace-b");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::create_dir_all(&other).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (local, _) = session_in(&paths, &cwd, "local");
        // 目标工作区里建两个会话，指针最后落在 newer 上
        let foreign = create_session_for_workspace(&paths, &other, Some("foreign")).unwrap();
        let newer = create_session_for_workspace(&paths, &other, Some("newer")).unwrap();
        assert_eq!(
            crate::state::active_session_id_for_workspace(&paths, &other).unwrap(),
            newer.id
        );

        // 默认拒绝跨工作区激活（按 id 或按名都一样）
        for arguments in [
            format!(r#"{{"session_id":"{}"}}"#, foreign.id),
            r#"{"name":"foreign"}"#.to_string(),
        ] {
            let error = registry_with_cross_session(&paths, &local, false)
                .call("session_activate", &arguments)
                .await
                .unwrap_err();
            assert!(
                error.to_string().contains("another workspace"),
                "跨工作区激活默认必须拒绝: {error}"
            );
        }
        assert_eq!(
            crate::state::active_session_id_for_workspace(&paths, &other).unwrap(),
            newer.id,
            "被拒绝的激活不能改目标工作区指针"
        );

        // 显式开启后：切换目标工作区的指针，不动本工作区
        let activated: Value = serde_json::from_str(
            &registry_with_cross_session(&paths, &local, true)
                .call(
                    "session_activate",
                    &format!(r#"{{"session_id":"{}"}}"#, foreign.id),
                )
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(activated["ok"], true, "{activated}");
        assert_eq!(activated["session_id"], foreign.id);
        assert_eq!(
            crate::state::active_session_id_for_workspace(&paths, &other).unwrap(),
            foreign.id,
            "激活后目标工作区指针必须指向 foreign"
        );
        assert_eq!(
            crate::state::active_session(&paths).unwrap().id,
            local.id,
            "跨工作区激活不能改本工作区的当前会话"
        );
    })
    .await;
}

/// 工具说明必须写清：创建不激活、会话以 id 为键、重名报候选、跨工作区要开关。
///
/// 两种语言都要覆盖：模型在中文界面下读到的是中文那份说明。
#[test]
fn session_control_descriptions_document_the_contract() {
    for description in [
        super::super::session_create::DESCRIPTION_EN,
        super::super::session_create::DESCRIPTION_ZH,
    ] {
        assert!(
            description.contains("session_id"),
            "创建工具说明必须点名 session_id: {description}"
        );
        assert!(
            description.contains("NOT activated") || description.contains("不会被激活"),
            "创建工具说明必须声明不自动激活: {description}"
        );
        assert!(
            description.contains("keyed by id") || description.contains("以 id 为键"),
            "创建工具说明必须声明会话以 id 为键: {description}"
        );
    }
    for description in [
        super::super::session_activate::DESCRIPTION_EN,
        super::super::session_activate::DESCRIPTION_ZH,
    ] {
        assert!(
            description.contains("session_id"),
            "激活工具说明必须点名 session_id: {description}"
        );
        assert!(
            description.contains("mesh.cross_session"),
            "激活工具说明必须写清跨工作区开关: {description}"
        );
        assert!(
            description.contains("ambiguous") || description.contains("重名"),
            "激活工具说明必须写清重名处理: {description}"
        );
        assert!(
            description.contains("New session") || description.contains("占位标题"),
            "激活工具说明必须排除占位标题: {description}"
        );
    }
}
