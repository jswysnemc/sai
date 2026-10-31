use super::*;

/// 【会话创建】【指定目录回归】无参数，返回无；只在目标目录登记，会话与两个启动指针均保持原样。
#[tokio::test]
async fn create_in_specified_workdir_preserves_both_workspace_pointers() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let current = root.path().join("source");
    let target = root.path().join("target");
    std::fs::create_dir_all(&current).unwrap();
    std::fs::create_dir_all(&target).unwrap();
    crate::runtime_cwd::scope(current.clone(), async {
        let (source, _) = session_in(&paths, &current, "source");
        let (prior, _) = session_in(&paths, &target, "prior");
        let source_pointer = locate_session_dirs(&paths, &source.id)
            .unwrap()
            .0
            .join("current");
        let target_pointer = locate_session_dirs(&paths, &prior.id)
            .unwrap()
            .0
            .join("current");
        let before_source = std::fs::read(&source_pointer).unwrap();
        let before_target = std::fs::read(&target_pointer).unwrap();
        let registry = registry_with_cross_session(&paths, &source, true);
        let output: Value = serde_json::from_str(
            &registry
                .call("session_create", &json!({"workdir":target}).to_string())
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(output["pointer_changed"], false);
        assert_eq!(output["started"], false);
        assert_eq!(
            output["workdir"],
            crate::platform::windows_path::canonicalize(&target)
                .unwrap()
                .display()
                .to_string()
        );
        assert_eq!(std::fs::read(source_pointer).unwrap(), before_source);
        assert_eq!(std::fs::read(target_pointer).unwrap(), before_target);
        assert_eq!(
            crate::state::active_session_id_for_workspace(&paths, &target).unwrap(),
            prior.id
        );
        let located = crate::state::list_all_sessions(&paths)
            .unwrap()
            .into_iter()
            .find(|s| s.info.id == output["session_id"].as_str().unwrap())
            .unwrap();
        assert_eq!(
            located.workspace_id,
            crate::state::workspace_id_for_path(&target)
        );
        assert_ne!(
            located.workspace_id,
            crate::state::workspace_id_for_path(&current)
        );
    })
    .await;
}

/// 【会话创建】【目录校验回归】无参数，返回无；缺少目录、不存在或文件路径均不能创建会话。
#[tokio::test]
async fn invalid_workdir_never_creates_or_switches_a_session() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let cwd = root.path().join("work");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::write(cwd.join("file"), "x").unwrap();
    crate::runtime_cwd::scope(cwd.clone(), async {
        let (session, _) = session_in(&paths, &cwd, "current");
        let registry = registry_for(&paths, &session);
        for args in [
            json!({}),
            json!({"workdir":""}),
            json!({"workdir":"missing"}),
            json!({"workdir":"file"}),
        ] {
            assert!(registry
                .call("session_create", &args.to_string())
                .await
                .is_err());
        }
        assert_eq!(crate::state::list_all_sessions(&paths).unwrap().len(), 1);
        assert_eq!(crate::state::active_session(&paths).unwrap().id, session.id);
    })
    .await;
}

/// 【会话创建】【相对目录回归】无参数，返回无；相对 workdir 以调用方目录解释，不修改进程工作目录。
#[tokio::test]
async fn relative_workdir_is_resolved_without_switching_the_caller() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let cwd = root.path().join("work");
    std::fs::create_dir_all(cwd.join("project")).unwrap();
    crate::runtime_cwd::scope(cwd.clone(), async {
        let (session, _) = session_in(&paths, &cwd, "current");
        let registry = registry_for(&paths, &session);
        let created: Value = serde_json::from_str(
            &registry
                .call("session_create", r#"{"workdir":"project"}"#)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            created["workdir"],
            cwd.join("project").display().to_string()
        );
        assert_eq!(crate::runtime_cwd::current_dir().unwrap(), cwd);
        let base = locate_session_dirs(&paths, created["session_id"].as_str().unwrap())
            .unwrap()
            .0;
        assert!(!base.join("current").exists());
    })
    .await;
}
