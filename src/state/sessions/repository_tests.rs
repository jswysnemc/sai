#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::sessions::workspace_repository::list_sessions;

    fn test_paths(root: PathBuf) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.jsonc"),
            skills_dir: root.join("config/skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish/sai.fish"),
            bash_hook_file: root.join("shell/bash-hook.sh"),
            zsh_hook_file: root.join("shell/zsh-hook.zsh"),
            powershell_hook_file: root.join("shell/powershell-hook.ps1"),
        }
    }

    #[test]
    fn explicit_active_session_uses_workspace_state_dir() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());

        let session = ensure_active_session(&paths).unwrap();
        let scope_dir = session_scope_dir(&paths).unwrap();

        assert_ne!(session.id, "default");
        assert_eq!(
            state_dir_for_session(&paths, &session.id).unwrap(),
            scope_dir.join("data").join(&session.id)
        );
    }

    #[test]
    fn create_session_switches_current_session() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());

        let session = create_session(&paths, Some("Work")).unwrap();
        let active = ensure_active_session(&paths).unwrap();

        assert_eq!(active.id, session.id);
        assert!(state_dir_for_session(&paths, &session.id)
            .unwrap()
            .ends_with(&session.id));
    }

    #[test]
    fn create_session_detached_keeps_current_session() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());

        let current = create_session(&paths, Some("Current")).unwrap();
        let detached = create_session_detached(&paths, Some("Detached")).unwrap();

        // 创建不激活：当前指针仍指原会话，新会话已入索引
        assert_eq!(ensure_active_session(&paths).unwrap().id, current.id);
        assert_ne!(detached.id, current.id);
        let listed = list_sessions(&paths).unwrap();
        assert!(listed
            .iter()
            .any(|session| session.id == detached.id && session.title == "Detached"));
    }

    #[test]
    fn switch_session_located_switches_foreign_workspace_pointer() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let other = temp.path().join("workspace-b");
        std::fs::create_dir_all(&other).unwrap();

        // 目标工作区里后建的会话抢走指针，跨工作区切回先建的那个
        let first = create_session_for_workspace(&paths, &other, Some("First")).unwrap();
        let second = create_session_for_workspace(&paths, &other, Some("Second")).unwrap();
        assert_eq!(
            crate::state::active_session_id_for_workspace(&paths, &other).unwrap(),
            second.id
        );

        let switched = switch_session_located(&paths, &first.id).unwrap();

        assert_eq!(switched.id, first.id);
        assert_eq!(
            crate::state::active_session_id_for_workspace(&paths, &other).unwrap(),
            first.id
        );
    }

    /// 【会话管理】【删除回退】删除当前会话只选择已有会话，删除最后一条后保持空列表。
    /// @returns 无；无外部参数
    #[test]
    fn deletion_selects_existing_session_without_creating_default() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let first = create_session(&paths, Some("First")).unwrap();
        let second = create_session(&paths, Some("Second")).unwrap();
        assert!(delete_session(&paths, &second.id).unwrap());
        assert_eq!(
            active_session_if_present(&paths).unwrap().unwrap().id,
            first.id
        );
        assert!(delete_session(&paths, &first.id).unwrap());
        assert!(list_sessions(&paths).unwrap().is_empty());
        assert!(active_session_if_present(&paths).unwrap().is_none());
    }

    #[test]
    fn touch_updates_new_session_title() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let session = create_session(&paths, None).unwrap();

        let scope_dir = session_scope_dir(&paths).unwrap();
        touch_session_with_message(&scope_dir, &session.id, "hello project world").unwrap();
        let updated = list_sessions(&paths)
            .unwrap()
            .into_iter()
            .find(|item| item.id == session.id)
            .unwrap();

        assert_eq!(updated.title, "hello project world");
    }
}
