//! 运行期间的设置提交与普通消息分流。

use super::super::stream_commands::{run_immediate_stream_command, StreamCommandContext};
use super::*;
use crate::agent::AgentMode;
use crate::config::ProviderConfig;

/// 【终端设置测试】【命令分流】参数：无；返回：无，验证模型和思考立即保存，不改变草稿与消息队列
#[test]
fn stream_commands_save_model_and_thinking_without_queuing_chat() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut provider = ProviderConfig::default_openai();
    provider.api_key = Some("test-key".into());
    provider.models = vec!["model-a".into(), "model-b".into()];
    provider.default_model = "model-a".into();
    provider.set_model_context_limit_tokens_for("model-b", Some(120_000));
    let config = AppConfig {
        active_provider: provider.id.clone(),
        providers: vec![provider],
        ..Default::default()
    };
    config.save(&paths).unwrap();
    let mut runtime = ReplRuntime::new(
        1000,
        crate::render::transcript::TranscriptRenderOptions {
            reasoning_mode: crate::render::ReasoningDisplayMode::Summary,
            tool_call_mode: crate::render::ToolCallDisplayMode::Summary,
        },
    );
    runtime.stream_draft.text = "kept draft".into();
    let state = crate::state::StateStore::new(&paths).unwrap();
    runtime.last_chrome = Some(crate::cli::repl_chrome::ReplChrome::from_runtime(
        &config,
        &paths,
        &state,
        AgentMode::Yolo,
    ));
    runtime.bind_live_model_settings(Default::default());
    let ctx =
        StreamCommandContext::capture(&paths, root.path().display().to_string(), AgentMode::Yolo);
    run_immediate_stream_command(&mut runtime, &ctx, "/model 2").unwrap();
    run_immediate_stream_command(&mut runtime, &ctx, "/think high").unwrap();
    let saved = AppConfig::load(&paths).unwrap();
    assert_eq!(saved.provider(None).unwrap().default_model, "model-b");
    assert_eq!(saved.provider(None).unwrap().thinking_level, "high");
    assert_eq!(runtime.stream_draft.text, "kept draft");
    let chrome = runtime.last_chrome.as_ref().unwrap();
    assert_eq!(chrome.model, "model-b");
    assert_eq!(chrome.thinking, "high");
    assert_eq!(chrome.context_window_tokens, 120_000);
    assert_eq!(runtime.queue_len(), 0);
    assert!(runtime.has_pending_settings());
    let pending = runtime.take_pending_settings();
    assert!(pending.clear_thinking_override);
    assert!(!runtime.has_pending_settings());
    run_immediate_stream_command(&mut runtime, &ctx, "/think invalid").unwrap();
    assert!(!runtime.has_pending_settings());
    assert_eq!(
        AppConfig::load(&paths)
            .unwrap()
            .provider(None)
            .unwrap()
            .thinking_level,
        "high"
    );
}
