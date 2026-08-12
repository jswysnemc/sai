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
    async fn wait_returns_finished_subagent_and_acknowledges_delivery() {
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
        // wait 已经把结果交给主模型，后台事件不得再次投递相同完成通知
        let notices = subagent_state::pending_finished_notices("default");
        assert!(!notices.iter().any(|notice| notice.id == subagent.id));
    }

    #[tokio::test]
    async fn wait_for_any_acknowledges_all_returned_notices() {
        let (first, _first_cancel) = subagent_state::create_subagent(
            "first wait target".to_string(),
            "general".to_string(),
            5,
        );
        let (second, _second_cancel) = subagent_state::create_subagent(
            "second wait target".to_string(),
            "general".to_string(),
            5,
        );
        for subagent in [&first, &second] {
            subagent_state::finish_subagent(
                &subagent.id,
                "completed",
                Some("done".to_string()),
                None,
                None,
            );
        }
        let (progress_tx, _progress_rx) = tokio::sync::mpsc::unbounded_channel();

        let result = wait_subagent(
            json!({"timeout_seconds": 5}),
            ToolProgress::new(progress_tx),
            "default",
        )
        .await
        .unwrap();

        let value = serde_json::from_str::<Value>(&result).unwrap();
        let returned_ids = value["finished"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| item["id"].as_str())
            .collect::<std::collections::HashSet<_>>();
        assert!(returned_ids.contains(first.id.as_str()));
        assert!(returned_ids.contains(second.id.as_str()));
        let notices = subagent_state::pending_finished_notices("default");
        assert!(!notices.iter().any(|notice| notice.id == first.id));
        assert!(!notices.iter().any(|notice| notice.id == second.id));
    }

    /// 【send】验证追加消息进入队列并返回最新快照。
    #[test]
    fn send_queues_message_for_running_subagent() {
        let owner = "send-owner";
        let (subagent, _cancel) = subagent_state::create_subagent_for_owner(
            owner,
            "send target".to_string(),
            "general".to_string(),
            5,
        );

        let result = subagent_send(
            json!({"subagent_id": subagent.id, "message": "补充要求"}),
            owner,
        )
        .unwrap();
        let value = serde_json::from_str::<Value>(&result).unwrap();

        assert_eq!(value["ok"], true);
        assert_eq!(value["subagent"]["pending_messages"], 1);
        // 缺 message 参数时报错
        assert!(subagent_send(json!({"subagent_id": subagent.id}), owner).is_err());
        // 其他会话不能向该子智能体发消息
        assert!(subagent_send(
            json!({"subagent_id": subagent.id, "message": "hi"}),
            "other-owner"
        )
        .is_err());
    }

    /// 【stop】验证 stop 只接受持久子智能体并透传 apply 标志。
    #[test]
    fn stop_requires_persistent_and_carries_apply_flag() {
        let owner = "stop-owner";
        let (one_shot, _one_shot_cancel) = subagent_state::create_subagent_for_owner(
            owner,
            "one shot".to_string(),
            "general".to_string(),
            5,
        );
        assert!(subagent_stop(json!({"subagent_id": one_shot.id}), owner).is_err());

        let (persistent, _cancel) = subagent_state::create_subagent_for_owner_goal(
            owner,
            None,
            "persistent".to_string(),
            "general".to_string(),
            0,
            true,
        );
        let result =
            subagent_stop(json!({"subagent_id": persistent.id, "apply": false}), owner).unwrap();
        let value = serde_json::from_str::<Value>(&result).unwrap();

        assert_eq!(value["ok"], true);
        assert_eq!(value["apply"], false);
        let stop = subagent_state::subagent_stop_requested(&persistent.id).unwrap();
        assert!(!stop.apply);
    }

    /// 【默认行为】验证不传 persistent 时创建的是一次性子智能体。
    #[test]
    fn start_defaults_to_one_shot_subagent() {
        let (subagent, _cancel) = subagent_state::create_subagent_for_owner(
            "default-behavior-owner",
            "plain".to_string(),
            "general".to_string(),
            3,
        );

        assert!(!subagent.persistent);
        assert_eq!(subagent.pending_messages, 0);
        assert_eq!(subagent.turns_completed, 0);
    }

    #[test]
    fn result_acknowledges_the_finished_notice() {
        let owner = "result-ack-owner";
        let (subagent, _cancel) = subagent_state::create_subagent_for_owner(
            owner,
            "result target".to_string(),
            "general".to_string(),
            5,
        );
        subagent_state::finish_subagent(
            &subagent.id,
            "completed",
            Some("done".to_string()),
            None,
            None,
        );

        let result = subagent_result(json!({"subagent_id": subagent.id}), owner).unwrap();
        let value = serde_json::from_str::<Value>(&result).unwrap();

        assert_eq!(value["subagent"]["result"], "done");
        assert!(subagent_state::pending_finished_notices(owner).is_empty());
    }
}
