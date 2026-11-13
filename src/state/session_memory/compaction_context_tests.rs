use super::*;
use crate::state::session_memory::{model::NewSessionMemory, repository::upsert_memory};
use crate::state::StateStore;

/// 【会话记忆】【压缩夹具】建立三轮历史和会话记忆；返回临时目录及存储。
fn fixture(summary: &str) -> (tempfile::TempDir, StateStore) {
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(&crate::paths::SaiPaths::for_tests(root.path())).unwrap();
    for id in ["old", "tail", "latest"] {
        store.start_turn(id, "new explicit request").unwrap();
        store.complete_turn(id, "completed work", None).unwrap();
    }
    upsert_memory(
        &store.conv_db,
        NewSessionMemory {
            session_id: store.session_id().into(),
            summary: summary.into(),
            last_summarized_turn_id: Some("latest".into()),
            last_summarized_seq: 3,
            checkpoint_id: None,
            source_turn_count: 3,
            token_estimate: 10,
        },
    )
    .unwrap();
    (root, store)
}

/// 【会话记忆】【压缩输入回归】记忆和原始历史共同进入压缩，原始要求优先。
#[test]
fn compaction_prompt_includes_memory_and_preserves_source_priority() {
    let (_root, store) = fixture("Important project constraint: keep the CLI offline.");
    let request = store.select_manual_compaction(0).unwrap().unwrap();
    let template = crate::config::PromptTemplatesConfig::default().compaction;
    let prompt = store
        .build_compaction_summary_prompt(&request, 30_000, &template, true)
        .unwrap();
    assert!(prompt.user.contains("<session-memory>"));
    assert!(prompt.user.contains("keep the CLI offline"));
    assert!(prompt.user.contains("take precedence"));
    assert!(prompt.user.contains("new explicit request"));
    assert!(prompt.total_chars() <= 30_000);
}

/// 【会话记忆】【开关回归】记忆关闭时不改变原有压缩输入。
#[test]
fn disabled_memory_is_not_sent_to_compaction() {
    let (_root, store) = fixture("must not be sent");
    let request = store.select_manual_compaction(0).unwrap().unwrap();
    let template = crate::config::PromptTemplatesConfig::default().compaction;
    let prompt = store
        .build_compaction_summary_prompt(&request, 30_000, &template, false)
        .unwrap();
    assert!(!prompt.user.contains("session-memory"));
    assert!(!prompt.user.contains("must not be sent"));
}

/// 【会话记忆】【自定义模板回归】模板没有记忆占位符也能收到补充材料。
#[test]
fn custom_template_still_receives_memory() {
    let (_root, store) = fixture("custom memory fact");
    let request = store.select_manual_compaction(0).unwrap().unwrap();
    let template = crate::config::PromptTemplateConfig {
        system: "Summarize".into(),
        user: "{{history}}".into(),
    };
    let prompt = store
        .build_compaction_summary_prompt(&request, 30_000, &template, true)
        .unwrap();
    assert!(prompt.user.contains("custom memory fact"));
    assert!(prompt.user.contains("new explicit request"));
}

/// 【会话记忆】【预算回归】大记忆受独立上限约束，小窗口优先保留历史。
#[test]
fn memory_context_is_bounded_and_skipped_when_budget_is_tight() {
    let (_root, store) = fixture(&"constraint ".repeat(10_000));
    let context =
        render_compaction_memory(&store.conv_db, store.session_id(), 100_000, 1_000).unwrap();
    assert!(context.chars().count() <= MAX_MEMORY_CONTEXT_CHARS);
    assert!(context.ends_with(SUFFIX));
    assert!(
        render_compaction_memory(&store.conv_db, store.session_id(), 2_000, 1_100)
            .unwrap()
            .is_empty()
    );
    let request = store.select_manual_compaction(0).unwrap().unwrap();
    let template = crate::config::PromptTemplatesConfig::default().compaction;
    let prompt = store
        .build_compaction_summary_prompt(&request, 30_000, &template, true)
        .unwrap();
    assert!(prompt.total_chars() <= 30_000);
    assert!(prompt.user.contains("new explicit request"));
}
