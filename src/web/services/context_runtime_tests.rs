use super::*;
use serde_json::json;

/// 【回复预览测试】【旧状态】写入旧版本记录，业务读取必须经过真实 Lua 插件
/// @param paths 隔离目录；name 为发送名称
/// @returns 无
fn write_recent(paths: &SaiPaths, name: &str) {
    let path = paths.state_dir.join("memes/sai/auto-send.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        json!({"last":{"library":"sai","id":"sha256:abcdef","name":{"zh":name,"en":""},
        "description":"图像","usage":"聊天","reason":"问候","sent_at":"2026-01-01T00:00:00+00:00"}})
        .to_string(),
    )
    .unwrap();
}

/// 【回复预览测试】【实时读取和禁用】Web 预览读取旧记录，但自动发送开关不会触发模型或投递
/// @returns 无；禁用插件立即移除该上下文
#[tokio::test]
async fn web_reply_preview_reads_lua_context_and_respects_disabling() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let state = StateStore::new(&paths).unwrap();
    state.init_files().unwrap();
    let mut config = AppConfig::default();
    config.plugins.memes.auto_send_enabled = true;
    write_recent(&paths, "已发送图片");
    let projection = project_context_runtime(
        &config,
        &paths,
        &state,
        root.path().to_str().unwrap(),
        AgentMode::Yolo,
    )
    .await
    .unwrap();
    assert!(projection.plugin_reply_context.contains("已发送图片"));
    assert!(projection.runtime_context.contains("plugin_reply_memes"));
    config.plugins.memes.enabled = false;
    let disabled = project_context_runtime(
        &config,
        &paths,
        &state,
        root.path().to_str().unwrap(),
        AgentMode::Yolo,
    )
    .await
    .unwrap();
    assert!(disabled.plugin_reply_context.is_empty());
}

/// 【回复预览测试】【同步快照】流式用量读取已载入正文，实时 Web 预览读取新的持久记录
/// @returns 无；同步路径不启动异步策略或重新注入旧原生资源
#[tokio::test]
async fn synchronous_reply_usage_uses_loaded_context_without_waiting_for_a_policy() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let state = StateStore::new(&paths).unwrap();
    state.init_files().unwrap();
    let config = AppConfig::default();
    let resource = crate::agent::plugin_context_updates(
        &std::collections::BTreeMap::from([("memes".into(), "loaded context".into())]),
        None,
        &[],
    )
    .unwrap()
    .unwrap();
    state.start_turn("cached-context", "hello").unwrap();
    state
        .set_provider_user_content("cached-context", &resource)
        .unwrap();
    state
        .complete_turn("cached-context", "reply", None)
        .unwrap();
    write_recent(&paths, "新的持久记录");
    let cached = project_cached_context_runtime(
        &config,
        &paths,
        &state,
        root.path().to_str().unwrap(),
        AgentMode::Yolo,
    )
    .unwrap();
    assert_eq!(cached.plugin_reply_context, "loaded context");
    let current = project_context_runtime(
        &config,
        &paths,
        &state,
        root.path().to_str().unwrap(),
        AgentMode::Yolo,
    )
    .await
    .unwrap();
    assert!(current.plugin_reply_context.contains("新的持久记录"));
}
