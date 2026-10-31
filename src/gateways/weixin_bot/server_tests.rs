use super::*;
use serde_json::json;

/// 【微信网关】【入站凭证回归】无参数，返回无；处理队列尚未消费时，批量消息已经保存最新凭证。
#[tokio::test]
async fn credentials_are_persisted_before_agent_queue_processing() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let client = WeixinBotClient::new(
        "http://127.0.0.1:1".into(),
        "".into(),
        "fixture".into(),
        None,
        false,
    );
    let (sender, mut receiver) = tokio::sync::mpsc::channel(8);
    let response = json!({"ret":0,"msgs":[
        {"message_type":1,"from_user_id":"first","context_token":"old","item_list":[{"type":1,"text_item":{"text":"one"}}]},
        {"message_type":1,"from_user_id":"first","context_token":"fresh","item_list":[{"type":1,"text_item":{"text":"two"}}]},
        {"message_type":1,"from_user_id":"other","context_token":"other-token","item_list":[{"type":1,"text_item":{"text":"three"}}]}
    ]});
    handle_updates(&paths, &client, &sender, &response)
        .await
        .unwrap();
    let context = ChannelContext::weixin("first".into(), None);
    let session_id = crate::gateways::session::gateway_session_id(&context);
    let saved = crate::gateways::channel_context::load_session_channel_context(
        &paths,
        &crate::gateways::workspace::gateway_workspace_path(&paths),
        &session_id,
    )
    .unwrap();
    assert!(
        matches!(saved, Some(ChannelContext::Weixin { context_token: Some(token), .. }) if token == "fresh")
    );
    assert_eq!(receiver.recv().await.unwrap().prompt, "one");
    assert_eq!(receiver.recv().await.unwrap().prompt, "two");
    assert_eq!(receiver.recv().await.unwrap().prompt, "three");
}
