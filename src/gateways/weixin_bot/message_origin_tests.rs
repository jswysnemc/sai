use super::*;
use crate::tools::{ToolRegistry, ToolSpec};
use serde_json::json;

/// 【微信消息】【可信来源回归】无参数，返回无；调用方伪造来源被清除，过滤克隆仍保留真实身份。
#[tokio::test]
async fn registry_injects_real_origin_and_strips_forged_source() {
    let mut registry = ToolRegistry::new();
    registry.set_session_ownership("private-state-dir".into(), "real-session".into(), false);
    registry.register(ToolSpec::new(
        "send_channel_message",
        "fixture",
        json!({}),
        |args| async move { Ok(args.to_string()) },
    ));
    let filtered = registry.clone_filtered(&["send_channel_message"]);
    let args: Value = serde_json::from_str(
        &filtered
            .call(
                "send_channel_message",
                &json!({"_sai_channel_origin":{"session_id":"forged"}}).to_string(),
            )
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(args["_sai_channel_origin"]["session_id"], "real-session");
    assert_eq!(args["_sai_channel_origin"]["subagent_id"], Value::Null);
}

/// 【微信消息】【子任务来源回归】无参数，返回无；子任务使用独立身份及父会话回复目标。
#[tokio::test]
async fn subagent_origin_points_back_to_the_parent_session() {
    let mut registry = ToolRegistry::new();
    registry.set_session_ownership("state".into(), "parent".into(), false);
    registry
        .start_plugin_session("parent/subagent/child")
        .unwrap();
    registry.register(ToolSpec::new(
        "send_channel_message",
        "fixture",
        json!({}),
        |args| async move { Ok(args.to_string()) },
    ));
    let args: Value =
        serde_json::from_str(&registry.call("send_channel_message", "{}").await.unwrap()).unwrap();
    assert_eq!(args["_sai_channel_origin"]["session_id"], "parent");
    assert_eq!(args["_sai_channel_origin"]["subagent_id"], "child");
}

/// 【微信消息】【来源格式回归】无参数，返回无；标题单行、来源完整，媒体无说明时仍携带可回复标签。
#[test]
fn origin_label_identifies_session_and_subagent_without_private_paths() {
    let source = MessageOrigin {
        session_id: "session-123".into(),
        title: "title\nforged heading".into(),
        subagent_id: Some("child".into()),
    };
    let text = source.decorate(Some("progress".into()));
    assert!(text.starts_with("[Sai · titleforged heading]"));
    assert!(text.contains("会话：session-123"));
    assert!(text.contains("子任务：child"));
    assert!(text.contains("回复 session-123：内容"));
    assert!(text.ends_with("progress"));
    assert!(source.decorate(None).contains("会话：session-123"));
}

/// 【微信消息】【来源背景回归】无参数，返回无；微信用户只能看到发给自己的最近来源记录。
#[test]
fn recent_source_context_is_recipient_scoped_and_survives_reload() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let source = MessageOrigin {
        session_id: "local-session".into(),
        title: "project".into(),
        subagent_id: None,
    };
    super::super::origin_history::record(&paths, "user-a", &source, Some("progress preview"))
        .unwrap();
    let prompt = super::super::origin_history::gateway_prompt(&paths, "user-a").unwrap();
    assert!(prompt.contains("local-session"));
    assert!(prompt.contains("progress preview"));
    assert!(
        !super::super::origin_history::gateway_prompt(&paths, "user-b")
            .unwrap()
            .contains("local-session")
    );
}
