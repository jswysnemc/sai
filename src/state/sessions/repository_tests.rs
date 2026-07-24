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
    fn default_session_uses_workspace_state_dir() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());

        let session = ensure_active_session(&paths).unwrap();
        let scope_dir = session_scope_dir(&paths).unwrap();

        assert_eq!(session.id, DEFAULT_SESSION_ID);
        assert_eq!(
            active_state_dir(&paths).unwrap(),
            scope_dir.join("data").join(DEFAULT_SESSION_ID)
        );
    }

    #[test]
    fn create_session_switches_current_session() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());

        let session = create_session(&paths, Some("Work")).unwrap();
        let active = ensure_active_session(&paths).unwrap();

        assert_eq!(active.id, session.id);
        assert!(active_state_dir(&paths).unwrap().ends_with(&session.id));
    }

    #[test]
    fn delete_active_session_switches_to_default() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let session = create_session(&paths, Some("Work")).unwrap();

        assert!(delete_session(&paths, &session.id).unwrap());

        assert_eq!(
            ensure_active_session(&paths).unwrap().id,
            DEFAULT_SESSION_ID
        );
    }

    #[test]
    fn allows_deleting_default_and_all_sessions() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let first = create_session(&paths, Some("First")).unwrap();
        let default_id = DEFAULT_SESSION_ID.to_string();
        let deleted = delete_sessions(&paths, &[first.id.clone(), default_id.clone()]).unwrap();
        assert!(deleted.contains(&first.id));
        assert!(deleted.contains(&default_id));
        // 删空后 ensure 会补回空白默认会话
        let active = ensure_active_session(&paths).unwrap();
        assert_eq!(active.id, DEFAULT_SESSION_ID);
        assert_eq!(list_sessions(&paths).unwrap().len(), 1);
    }

    #[test]
    fn deletes_multiple_sessions_with_one_index_update() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let first = create_session(&paths, Some("First")).unwrap();
        let second = create_session(&paths, Some("Second")).unwrap();

        let deleted = delete_sessions(&paths, &[first.id.clone(), second.id.clone()]).unwrap();
        let remaining = list_sessions(&paths).unwrap();

        assert_eq!(deleted.len(), 2);
        assert!(remaining
            .iter()
            .all(|session| session.id == DEFAULT_SESSION_ID));
        assert_eq!(
            ensure_active_session(&paths).unwrap().id,
            DEFAULT_SESSION_ID
        );
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

    #[test]
    fn migrates_legacy_sessions_into_workspace_scope() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let legacy_session = SessionInfo {
            id: "session_old".to_string(),
            title: "Old work".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let legacy_sessions_dir = paths.state_dir.join("sessions");
        std::fs::create_dir_all(legacy_sessions_dir.join("data/session_old")).unwrap();
        std::fs::create_dir_all(&paths.state_dir).unwrap();
        std::fs::write(
            legacy_sessions_dir.join("index.json"),
            serde_json::to_string_pretty(&vec![legacy_session.clone()]).unwrap(),
        )
        .unwrap();
        std::fs::write(legacy_sessions_dir.join("current"), "session_old\n").unwrap();
        std::fs::write(
            legacy_sessions_dir.join("data/session_old/usage.json"),
            "old session usage",
        )
        .unwrap();
        std::fs::write(paths.state_dir.join("conversation.jsonl"), "legacy default").unwrap();

        let active = ensure_active_session(&paths).unwrap();
        let scope_dir = session_scope_dir(&paths).unwrap();

        assert_eq!(active.id, "session_old");
        assert!(sessions_file(&scope_dir).exists());
        assert_eq!(
            std::fs::read_to_string(current_session_file(&scope_dir)).unwrap(),
            "session_old\n"
        );
        assert_eq!(
            std::fs::read_to_string(scope_dir.join("data/session_old/usage.json")).unwrap(),
            "old session usage"
        );
        assert_eq!(
            std::fs::read_to_string(
                session_state_dir(&scope_dir, DEFAULT_SESSION_ID).join("conversation.jsonl")
            )
            .unwrap(),
            "legacy default"
        );
    }
}
