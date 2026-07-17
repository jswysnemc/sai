#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::SaiPaths;

    fn test_paths(root: &Path) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.jsonc"),
            skills_dir: root.join("config/skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish/conf.d/sai.fish"),
            bash_hook_file: root.join("config/shell/bash-hook.sh"),
            zsh_hook_file: root.join("config/shell/zsh-hook.zsh"),
            powershell_hook_file: root.join("config/shell/powershell-hook.ps1"),
        }
    }

    #[test]
    fn edit_lines_replaces_inclusive_range() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let config = AppConfig::default();
        let kb = KnowledgeBase::new(config, paths).unwrap();
        let source = temp.path().join("note.md");
        std::fs::write(&source, "one\ntwo\nthree\n").unwrap();
        kb.import_file(&source, "notes/note.md").unwrap();

        let result = kb.edit_lines("notes/note.md", 2, 2, "TWO\nTWO-B").unwrap();

        assert_eq!(result.old_line_count, 3);
        assert_eq!(result.new_line_count, 4);
        assert!(!result.semantic_refreshed);
        let edited =
            std::fs::read_to_string(kb.existing_file_path("notes/note.md").unwrap()).unwrap();
        assert_eq!(edited, "one\nTWO\nTWO-B\nthree\n");
        let chunks: i64 = kb
            .semantic_conn()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM semantic_chunks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(chunks, 0);
    }

    #[test]
    fn edit_lines_empty_replacement_deletes_range() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let config = AppConfig::default();
        let kb = KnowledgeBase::new(config, paths).unwrap();
        let source = temp.path().join("note.md");
        std::fs::write(&source, "one\ntwo\nthree").unwrap();
        kb.import_file(&source, "note.md").unwrap();

        let result = kb.edit_lines("note.md", 2, 3, "").unwrap();

        assert_eq!(result.old_line_count, 3);
        assert_eq!(result.new_line_count, 1);
        let edited = std::fs::read_to_string(kb.existing_file_path("note.md").unwrap()).unwrap();
        assert_eq!(edited, "one");
    }

    #[test]
    fn edit_lines_rejects_out_of_range() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let config = AppConfig::default();
        let kb = KnowledgeBase::new(config, paths).unwrap();
        let source = temp.path().join("note.md");
        std::fs::write(&source, "one\n").unwrap();
        kb.import_file(&source, "note.md").unwrap();

        let error = kb.edit_lines("note.md", 2, 2, "two").unwrap_err();

        assert!(error.to_string().contains("out of range"));
    }
    #[tokio::test]
    async fn knowledge_base_starts_empty_without_default_materials() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let config = AppConfig::default();
        let kb = KnowledgeBase::new(config.clone(), paths.clone()).unwrap();
        assert!(kb.list().unwrap().is_empty());

        let source = temp.path().join("notes.md");
        std::fs::write(&source, "# hello\n").unwrap();
        let added = kb.add_path(&source).await.unwrap();
        assert_eq!(added, vec!["notes.md".to_string()]);
        assert_eq!(kb.list().unwrap().len(), 1);

        kb.remove("notes.md").unwrap();
        assert!(kb.list().unwrap().is_empty());
    }

}
