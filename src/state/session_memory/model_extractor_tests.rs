use super::*;
use crate::state::session_memory::repository::load_memory;

/// 【会话记忆】【测试夹具】创建隔离数据库；返回临时目录和数据库。
fn fixture() -> (tempfile::TempDir, ConversationDb) {
    let root = tempfile::tempdir().unwrap();
    let db = ConversationDb::open(root.path()).unwrap();
    (root, db)
}

/// 【会话记忆】【测试配置】使用零间隔测试模型输入；返回预算配置。
fn config() -> ModelExtractionConfig {
    ModelExtractionConfig {
        min_new_tokens: 0,
        ..ModelExtractionConfig::for_context_limit(10_000)
    }
}

/// 【会话记忆】【预算回归】首次超过间隔后仍遵守独立提示词预算。
#[test]
fn model_extraction_input_respects_prompt_budget() {
    let (_root, db) = fixture();
    db.start_turn("turn_1", &"user ".repeat(1_000)).unwrap();
    db.complete_turn("turn_1", &"assistant ".repeat(1_000), None)
        .unwrap();
    let config = ModelExtractionConfig {
        min_new_tokens: 0,
        max_prompt_chars: 1_200,
        max_source_chars: 900,
        max_summary_chars: 400,
        failure_threshold: 3,
        disable_seconds: 3_600,
    };
    let input = prepare_model_extraction_input(&db, "default", &config)
        .unwrap()
        .unwrap();
    assert!(input.prompt_chars <= config.max_prompt_chars);
    assert!(input.prompt.contains("Recent source turns"));
}

/// 【会话记忆】【提交回归】保存模型记忆并更新累计边界，没有新增内容时不再次请求。
#[test]
fn model_extraction_summary_updates_memory_and_interval() {
    let (_root, db) = fixture();
    db.start_turn("turn_1", "new task").unwrap();
    db.complete_turn("turn_1", "new answer", None).unwrap();
    let config = config();
    let input = prepare_model_extraction_input(&db, "default", &config)
        .unwrap()
        .unwrap();
    assert!(apply_model_extraction_summary(
        &db,
        "default",
        &input,
        "model memory summary",
        &config
    )
    .unwrap());
    let memory = load_memory(&db, "default").unwrap().unwrap();
    assert_eq!(memory.summary, "model memory summary");
    assert_eq!(memory.last_summarized_seq, 1);
    assert_eq!(memory.source_turn_count, 1);
    assert_eq!(memory.consecutive_failures, 0);
    assert!(prepare_model_extraction_input(&db, "default", &config)
        .unwrap()
        .is_none());
}

/// 【会话记忆】【默认阈值】短轮次以及尚在运行的轮次不触发后台模型。
#[test]
fn default_interval_skips_short_and_running_turns() {
    let (_root, db) = fixture();
    db.start_turn("turn_1", &" x".repeat(40_001)).unwrap();
    let config = ModelExtractionConfig::for_context_limit(128_000);
    assert_eq!(config.min_new_tokens, 40_000);
    assert!(prepare_model_extraction_input(&db, "default", &config)
        .unwrap()
        .is_none());
    db.complete_turn("turn_1", "", None).unwrap();
    assert!(prepare_model_extraction_input(&db, "default", &config)
        .unwrap()
        .is_some());
}

/// 【会话记忆】【失败回归】失败不推进间隔，三次连续失败触发熔断。
#[test]
fn failed_extractions_preserve_interval_and_open_circuit() {
    let (_root, db) = fixture();
    db.start_turn("turn_1", "task").unwrap();
    db.complete_turn("turn_1", "answer", None).unwrap();
    let config = config();
    for _ in 0..3 {
        let input = prepare_model_extraction_input(&db, "default", &config)
            .unwrap()
            .unwrap();
        record_model_extraction_failure(&db, "default", "provider failed", &input, &config)
            .unwrap();
    }
    let memory = load_memory(&db, "default").unwrap().unwrap();
    assert_eq!(memory.consecutive_failures, 3);
    assert!(prepare_model_extraction_input(&db, "default", &config)
        .unwrap()
        .is_none());
}

/// 【会话记忆】【过期回归】压缩重置累计边界后，旧后台结果不得覆写记忆。
#[test]
fn reset_boundary_rejects_in_flight_result() {
    let (_root, db) = fixture();
    db.start_turn("turn_1", "task").unwrap();
    db.complete_turn("turn_1", "answer", None).unwrap();
    let config = config();
    let input = prepare_model_extraction_input(&db, "default", &config)
        .unwrap()
        .unwrap();
    db.with_conn(|conn| {
        extraction_interval::reset_boundary(conn, "default", &input.ticket.boundary)
    })
    .unwrap();
    assert!(
        !apply_model_extraction_summary(&db, "default", &input, "outdated summary", &config)
            .unwrap()
    );
    assert!(load_memory(&db, "default").unwrap().is_none());
}

/// 【会话记忆】【空摘要回归】校验失败释放租约，后续可以重试。
#[test]
fn empty_summary_releases_lease_without_advancing_boundary() {
    let (_root, db) = fixture();
    db.start_turn("turn_1", "task").unwrap();
    db.complete_turn("turn_1", "answer", None).unwrap();
    let config = config();
    let input = prepare_model_extraction_input(&db, "default", &config)
        .unwrap()
        .unwrap();
    assert!(apply_model_extraction_summary(&db, "default", &input, "", &config).is_err());
    assert!(prepare_model_extraction_input(&db, "default", &config)
        .unwrap()
        .is_some());
}
