#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_action_defaults_to_none() {
        let value = json!({});

        assert_eq!(optional_string_arg(&value, "action").unwrap(), None);
    }

    #[test]
    fn rejects_non_string_optional_argument() {
        let value = json!({"action": 123});

        assert!(optional_string_arg(&value, "action").is_err());
    }

    #[test]
    fn reads_required_string_argument() {
        let value = json!({"prompt": " inspect code "});

        assert_eq!(string_arg(&value, "prompt").unwrap(), "inspect code");
    }

    #[test]
    fn rejects_empty_required_string_argument() {
        let value = json!({"prompt": "   "});

        assert!(string_arg(&value, "prompt").is_err());
    }

    #[test]
    fn summarizes_prompt_text() {
        assert_eq!(
            summarize_prompt("  inspect   this code\nnow "),
            "inspect this code now"
        );
    }

    #[test]
    fn unified_custom_agent_keeps_empty_tool_selection() {
        let mut config = AppConfig::default();
        let profile = crate::config::AgentProfile {
            id: "review".to_string(),
            name: "Review".to_string(),
            ..crate::config::AgentProfile::default()
        };
        config.agents.push(profile.clone());

        assert!(!inherits_default_tools(&config, &profile));
    }

    #[test]
    fn builtin_and_legacy_agents_inherit_empty_tool_selection() {
        let config = AppConfig::default();
        let builtin = crate::config::AgentProfile {
            id: "general".to_string(),
            name: "General".to_string(),
            ..crate::config::AgentProfile::default()
        };
        let legacy = crate::config::AgentProfile {
            id: "legacy".to_string(),
            name: "Legacy".to_string(),
            ..crate::config::AgentProfile::default()
        };

        assert!(inherits_default_tools(&config, &builtin));
        assert!(inherits_default_tools(&config, &legacy));
    }

    #[tokio::test]
    async fn wait_returns_finished_subagent_without_acknowledging_delivery() {
        let (subagent, _cancel) =
            subagent_state::create_subagent("wait target".to_string(), "general".to_string(), 5);
        subagent_state::finish_subagent(
            &subagent.id,
            "completed",
            Some("done".to_string()),
            None,
            None,
        );
        let (progress_tx, _progress_rx) = tokio::sync::mpsc::unbounded_channel();

        let result = wait_subagent(
            json!({"subagent_id": subagent.id, "timeout_seconds": 5}),
            ToolProgress::new(progress_tx),
            "default",
        )
        .await
        .unwrap();

        let value = serde_json::from_str::<Value>(&result).unwrap();
        assert_eq!(value["ok"], true);
        assert_eq!(value["subagent"]["status"], "completed");
        // wait 只返回查询结果,完成通知必须等到主模型请求成功后再确认
        let notices = subagent_state::pending_finished_notices("default");
        assert!(notices.iter().any(|notice| notice.id == subagent.id));
    }
}
