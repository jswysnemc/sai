#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::paths::SaiPaths;

    fn test_paths(temp: &tempfile::TempDir) -> SaiPaths {
        SaiPaths {
            config_dir: temp.path().join("config"),
            config_file: temp.path().join("config/config.jsonc"),
            secrets_file: temp.path().join("config/secrets.jsonc"),
            skills_dir: temp.path().join("config/skills"),
            data_dir: temp.path().join("data"),
            cache_dir: temp.path().join("cache"),
            state_dir: temp.path().join("state"),
            pictures_dir: temp.path().join("pictures"),
            fish_hook_file: temp.path().join("fish/sai.fish"),
            bash_hook_file: temp.path().join("shell/bash-hook.sh"),
            zsh_hook_file: temp.path().join("shell/zsh-hook.zsh"),
            powershell_hook_file: temp.path().join("shell/powershell-hook.ps1"),
        }
    }

    #[test]
    fn remembers_and_recalls_fact() {
        let temp = tempfile::tempdir().unwrap();
        let config = AppConfig::default();
        let paths = test_paths(&temp);
        let store = MemoryStore::new(&config, &paths);
        store
            .remember_fact("Niri 输入法需要 XMODIFIERS", "test")
            .unwrap();
        let result = store.recall_memories("Niri XMODIFIERS", 5, false).unwrap();
        assert!(result.to_string().contains("XMODIFIERS"));
    }

    #[test]
    fn writes_markdown_and_fts_index() {
        let temp = tempfile::tempdir().unwrap();
        let config = AppConfig::default();
        let paths = test_paths(&temp);
        let store = MemoryStore::new(&config, &paths);
        let id = store
            .remember_fact("Niri 是基于 Smithay 的 Wayland 合成器", "test")
            .unwrap();
        assert!(id > 0);
        let md = config
            .active_persona_memory_data_dir(&paths)
            .join("memory/files/facts")
            .join(format!("{id}.md"));
        assert!(md.is_file(), "expected markdown at {}", md.display());
        let body = std::fs::read_to_string(&md).unwrap();
        assert!(body.contains("Niri"));
        assert!(body.contains("kind: facts"));
        let result = store.recall_memories("Smithay Wayland", 5, false).unwrap();
        assert!(result.to_string().contains("Smithay"));
    }

    #[test]
    fn reset_all_clears_facts_and_episodes() {
        let temp = tempfile::tempdir().unwrap();
        let config = AppConfig::default();
        let paths = test_paths(&temp);
        let store = MemoryStore::new(&config, &paths);
        store
            .remember_fact("Niri 输入法需要 XMODIFIERS", "test")
            .unwrap();
        store
            .remember_pending_event("完成提交", "提交完成。提交：3a85e86。工作区已干净。")
            .unwrap();
        store.flush_pending_events().unwrap();

        let before = store.recall_memories("提交 XMODIFIERS", 5, false).unwrap();
        assert!(!before["facts"].as_array().unwrap().is_empty());
        assert!(!before["episodes"].as_array().unwrap().is_empty());

        store.reset_all(false).unwrap();

        let after = store.recall_memories("提交 XMODIFIERS", 5, false).unwrap();
        assert!(after["facts"].as_array().unwrap().is_empty());
        assert!(after["episodes"].as_array().unwrap().is_empty());
    }

    #[test]
    fn distills_pending_events_into_short_episodes() {
        let temp = tempfile::tempdir().unwrap();
        let config = AppConfig::default();
        let paths = test_paths(&temp);
        let store = MemoryStore::new(&config, &paths);
        store
            .remember_pending_event(
                "在某轮更改中,让todo在输入框上面常驻显示",
                "## 原因\n\n1. 位置不对\n2. 完成后不显示\n\n```\nnpm run build\n```\n\n已挪到输入框正上方并重建前端。",
            )
            .unwrap();
        store.flush_pending_events().unwrap();
        let result = store.recall_past_events("todo 输入框", 5).unwrap();
        let episodes = result["episodes"].as_array().unwrap();
        assert_eq!(episodes.len(), 1);
        let content = episodes[0]["content"].as_str().unwrap();
        assert!(content.contains("todo") || content.contains("输入框"));
        assert!(!content.contains("npm run build"));
        assert!(content.chars().count() < 320);
    }

    #[test]
    fn skips_greeting_pending_events() {
        let temp = tempfile::tempdir().unwrap();
        let config = AppConfig::default();
        let paths = test_paths(&temp);
        let store = MemoryStore::new(&config, &paths);
        store.remember_pending_event("你好", "在呢").unwrap();
        store.flush_pending_events().unwrap();
        let result = store.recall_past_events("你好", 5).unwrap();
        assert!(result["episodes"].as_array().unwrap().is_empty());
    }

    #[test]
    fn evicted_context_can_be_cleared() {
        let temp = tempfile::tempdir().unwrap();
        let config = AppConfig::default();
        let paths = test_paths(&temp);
        let store = MemoryStore::new(&config, &paths);
        store
            .remember_evicted_turns(&[EvictedTurn {
                timestamp: "now".to_string(),
                role: "user".to_string(),
                content: "旧上下文 输入法".to_string(),
            }])
            .unwrap();
        assert!(store
            .search_evicted_context("输入法", 5)
            .unwrap()
            .to_string()
            .contains("旧上下文"));
        store.clear_evicted_context().unwrap();
        assert!(!store
            .search_evicted_context("输入法", 5)
            .unwrap()
            .to_string()
            .contains("旧上下文"));
    }
}
