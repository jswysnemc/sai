use super::{
    memes_support::*,
    services_support::{ModelFixture, ModelReply},
};
use crate::{
    agent::{Agent, AgentEvent, AgentMode},
    paths::SaiPaths,
    state::{StateStore, TurnStatus},
    tools::ToolRegistry,
};
use serde_json::Value;
use std::sync::{atomic::Ordering, Arc};

/// 【表情 Agent 测试】【真实装配】在正式 Agent 中加载实际表情包并绑定可观察的文件宿主
/// @param root 临时目录；fixture 为本地模型服务；host 为文件宿主；mode 为权限模式
/// @returns 完整 Agent 和状态存储
fn agent(
    root: &std::path::Path,
    fixture: &ModelFixture,
    host: Arc<MemeHost>,
    mode: AgentMode,
) -> (Agent, StateStore) {
    let paths = SaiPaths::for_tests(root);
    let config = fixture.config("configured-default");
    let state = StateStore::new(&paths).unwrap();
    state.init_files().unwrap();
    let mut registry = ToolRegistry::new();
    registry.configure_plugin_model(&config, &paths);
    let mut plugin = descriptor(root, &config);
    plugin.setting.settings["auto_send_enabled"] = serde_json::json!(true);
    plugin.setting.settings["auto_send_probability"] = serde_json::json!(1.0);
    crate::plugins::registry::register_descriptor(&mut registry, plugin, host, false).unwrap();
    (
        Agent::new(
            config,
            &paths,
            state.clone(),
            fixture.client("chosen-client", &paths),
            registry,
            mode,
        )
        .unwrap(),
        state,
    )
}

/// 【表情 Agent 测试】【完整自动链路】准备请求使用当前模型，正文完成后投递，下一轮知道发送事件
/// @returns 无；正文、图片、状态和上下文资源各自只提交一次
#[tokio::test]
async fn memes_agent_runs_prepare_reply_delivery_and_following_context() {
    let fixture = ModelFixture::start(vec![
        ModelReply::text(r#"{"send":true,"id":"abc","confidence":1,"reason":"问候"}"#),
        ModelReply::text("first answer"),
        ModelReply::text(r#"{"send":false}"#),
        ModelReply::text("second answer"),
    ])
    .await;
    let root = tempfile::tempdir().unwrap();
    seed(
        root.path(),
        "builtin",
        vec![item("sha256:abcdef", "images/base.png", "企鹅")],
    );
    let host = MemeHost::new(root.path());
    let (mut agent, state) = agent(root.path(), &fixture, host.clone(), AgentMode::Yolo);
    let mut external = 0;
    crate::runtime_cwd::scope(root.path().to_path_buf(), async {
        let first = agent
            .chat_stream_with_images("Linux 你好", vec![], Some("meme-turn-1".into()), |event| {
                if matches!(event, AgentEvent::ExternalOutput) {
                    external += 1;
                    assert!(host.displays.lock().unwrap().is_empty());
                    assert_eq!(
                        state.load_turns().unwrap().last().unwrap().status,
                        TurnStatus::Completed
                    );
                }
                Ok(())
            })
            .await
            .unwrap();
        assert_eq!(first.content, "first answer");
        assert_eq!(host.displays.lock().unwrap().len(), 1);
        let saved: Value = serde_json::from_slice(
            &std::fs::read(root.path().join("recent/sai/auto-send.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(saved["last"]["id"], "sha256:abcdef");
        assert_eq!(
            agent
                .chat_stream_with_images(
                    "刚才那张表情",
                    vec![],
                    Some("meme-turn-2".into()),
                    |_| Ok(())
                )
                .await
                .unwrap()
                .content,
            "second answer"
        );
    })
    .await;
    assert_eq!(external, 1);
    assert_eq!(host.displays.lock().unwrap().len(), 1);
    let requests = fixture.requests();
    assert_eq!(requests.len(), 4);
    assert!(requests
        .iter()
        .all(|request| request["model"] == "chosen-client"));
    assert!(requests[1]["messages"].to_string().contains("计划发送表情"));
    assert!(requests[3]["messages"]
        .to_string()
        .contains("plugin_reply_memes"));
    assert!(requests[3]["messages"]
        .to_string()
        .contains("上一轮回复文字发送后"));
    assert!(state
        .load_turns()
        .unwrap()
        .iter()
        .all(|turn| turn.status == TurnStatus::Completed));
}

/// 【表情 Agent 测试】【附属错误隔离】终端显示失败不改变已经完成的主回复，也不伪造发送记录
/// @returns 无；主轮次保持 completed，图片记录缺失
#[tokio::test]
async fn memes_agent_delivery_failure_keeps_the_main_reply_completed() {
    let fixture = ModelFixture::start(vec![
        ModelReply::text(r#"{"send":true,"id":"abc","confidence":1}"#),
        ModelReply::text("answer remains valid"),
    ])
    .await;
    let root = tempfile::tempdir().unwrap();
    seed(
        root.path(),
        "builtin",
        vec![item("sha256:abcdef", "images/base.png", "企鹅")],
    );
    let host = MemeHost::new(root.path());
    host.fail_display.store(true, Ordering::SeqCst);
    let (mut agent, state) = agent(root.path(), &fixture, host, AgentMode::Yolo);
    let result = crate::runtime_cwd::scope(
        root.path().to_path_buf(),
        agent.chat_stream_with_images("Linux", vec![], Some("failed-image".into()), |_| Ok(())),
    )
    .await
    .unwrap();
    assert_eq!(result.content, "answer remains valid");
    assert_eq!(state.load_turns().unwrap()[0].status, TurnStatus::Completed);
    assert!(!root.path().join("recent").exists());
}

/// 【表情 Agent 测试】【热切换计划模式】正文结束前降为只读时丢弃准备结果，不执行任何图片或记录写入
/// @returns 无；只有主模型回复完成
#[tokio::test]
async fn memes_agent_rechecks_live_mode_before_delivery() {
    let fixture = ModelFixture::start(vec![
        ModelReply::text(r#"{"send":true,"id":"abc","confidence":1}"#),
        ModelReply::text("answer"),
    ])
    .await;
    let root = tempfile::tempdir().unwrap();
    seed(
        root.path(),
        "builtin",
        vec![item("sha256:abcdef", "images/base.png", "企鹅")],
    );
    let host = MemeHost::new(root.path());
    let (mut agent, _) = agent(root.path(), &fixture, host.clone(), AgentMode::Yolo);
    let mode = agent.live_mode_handle();
    crate::runtime_cwd::scope(
        root.path().to_path_buf(),
        agent.chat_stream_with_images("Linux", vec![], Some("mode-switch".into()), |event| {
            if matches!(event, AgentEvent::FlushContent) {
                mode.store(AgentMode::Plan.as_u8(), Ordering::SeqCst);
            }
            Ok(())
        }),
    )
    .await
    .unwrap();
    assert!(host.displays.lock().unwrap().is_empty());
    assert!(!root.path().join("recent").exists());
}

/// 【表情 Agent 测试】【只读模式】计划模式直接进行主回复，不调用自动发送决策模型
/// @returns 无；只收到一个主模型请求
#[tokio::test]
async fn memes_agent_plan_mode_never_prepares_delivery() {
    let fixture = ModelFixture::start(vec![ModelReply::text("plan answer")]).await;
    let root = tempfile::tempdir().unwrap();
    seed(
        root.path(),
        "builtin",
        vec![item("sha256:abcdef", "images/base.png", "企鹅")],
    );
    let host = MemeHost::new(root.path());
    let (mut agent, _) = agent(root.path(), &fixture, host.clone(), AgentMode::Plan);
    crate::runtime_cwd::scope(
        root.path().to_path_buf(),
        agent.chat_stream_with_images("Linux", vec![], Some("plan-mode".into()), |_| Ok(())),
    )
    .await
    .unwrap();
    assert_eq!(fixture.requests().len(), 1);
    assert!(host.displays.lock().unwrap().is_empty());
}

/// 【表情 Agent 测试】【准备错误隔离】损坏的旧状态不能阻止主模型完成回复
/// @returns 无；只有一次主请求，错误状态保持原样且没有图片投递
#[tokio::test]
async fn memes_agent_preparation_failure_keeps_the_main_reply_completed() {
    let fixture = ModelFixture::start(vec![ModelReply::text("main answer")]).await;
    let root = tempfile::tempdir().unwrap();
    let recent = root.path().join("recent/sai/auto-send.json");
    std::fs::create_dir_all(recent.parent().unwrap()).unwrap();
    std::fs::write(&recent, b"{broken").unwrap();
    let host = MemeHost::new(root.path());
    let (mut agent, state) = agent(root.path(), &fixture, host.clone(), AgentMode::Yolo);
    let reply = crate::runtime_cwd::scope(
        root.path().to_path_buf(),
        agent.chat_stream_with_images("hello", vec![], Some("prepare-failure".into()), |_| Ok(())),
    )
    .await
    .unwrap();
    assert_eq!(reply.content, "main answer");
    assert_eq!(state.load_turns().unwrap()[0].status, TurnStatus::Completed);
    assert_eq!(fixture.requests().len(), 1);
    assert!(host.displays.lock().unwrap().is_empty());
    assert_eq!(std::fs::read(recent).unwrap(), b"{broken");
}
