use super::{context_resource_update, plugin_context_snapshot, plugin_context_updates};
use crate::llm::ChatMessage;
use std::collections::BTreeMap;

/// 【回复上下文测试】【固定命名空间】插件 ID 不能覆盖同名核心资源
/// @returns 无；核心 Goal 记录保持独立
#[test]
fn reply_context_resources_never_use_a_core_name() {
    let contexts = BTreeMap::from([("goal".into(), "plugin text".into())]);
    let update = plugin_context_updates(&contexts, None, &[])
        .unwrap()
        .unwrap();
    assert!(update.contains("name=\"plugin_reply_goal\""));
    assert!(!update.contains("name=\"goal\""));
    assert_eq!(plugin_context_snapshot(Some(&update), &[]), contexts);
}

/// 【回复上下文测试】【历史清理】已停用插件和原生表情资源都产生空更新，旧正文不再作为当前上下文
/// @returns 无；清理后的资源不重复更新
#[test]
fn reply_context_clears_disabled_plugins_and_the_legacy_meme_resource() {
    let current = BTreeMap::from([("memes".into(), "previous meme".into())]);
    let legacy = context_resource_update("last_auto_meme", "legacy meme", None, &[])
        .unwrap()
        .unwrap();
    let active = plugin_context_updates(&current, None, &[])
        .unwrap()
        .unwrap();
    let history = vec![ChatMessage::plain("user", format!("{legacy}\n{active}"))];
    let cleared = plugin_context_updates(&BTreeMap::new(), None, &history)
        .unwrap()
        .unwrap();
    assert!(cleared.contains("name=\"last_auto_meme\""));
    assert!(cleared.contains("name=\"plugin_reply_memes\""));
    assert!(!cleared.contains("previous meme") && !cleared.contains("legacy meme"));
    let mut history = history;
    history.push(ChatMessage::plain("user", cleared));
    assert!(plugin_context_updates(&BTreeMap::new(), None, &history)
        .unwrap()
        .is_none());
    assert_eq!(plugin_context_snapshot(None, &history)["memes"], "");
}

/// 【回复上下文测试】【摘要恢复】完整资源进入压缩摘要后可以读取，正文被移除后重新注入
/// @returns 无；同一资源在正文完整时不重复注入
#[test]
fn reply_context_survives_checkpoint_projection_and_recovers_missing_bodies() {
    let contexts = BTreeMap::from([("sample".into(), "stable context".into())]);
    let initial = plugin_context_updates(&contexts, None, &[])
        .unwrap()
        .unwrap();
    assert!(plugin_context_updates(&contexts, Some(&initial), &[])
        .unwrap()
        .is_none());
    let only_state = initial.split("<context-resource name=").next().unwrap();
    assert!(plugin_context_snapshot(Some(only_state), &[]).is_empty());
    assert!(plugin_context_updates(&contexts, Some(only_state), &[])
        .unwrap()
        .unwrap()
        .contains("stable context"));
}
