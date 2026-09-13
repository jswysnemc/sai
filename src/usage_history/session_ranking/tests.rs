use super::*;
use crate::llm::Usage;
use crate::usage_history::{get_stats, record_model_call, UsageRecordInput};
use serde_json::json;

/// 【用量统计】【排行测试】构造具有确定身份和缓存用量的请求记录。
/// 参数: `session` 为会话，`workspace` 为可选工作区，`input` 为输入总量，`cached` 为缓存读取量
/// 返回: 用于聚合的调用记录
fn record(session: Option<&str>, workspace: Option<&str>, input: u64, cached: u64) -> UsageRecord {
    serde_json::from_value(json!({
        "id": "record", "created_at": 100, "completed_at": 101, "duration_ms": 1000,
        "source": "chat", "operation": "message", "provider_id": "anthropic",
        "provider_name": "Provider", "model": "claude", "status": "success",
        "usage_source": "provider_reported", "input_tokens": input, "output_tokens": 10,
        "total_tokens": input + 10, "cache_read_tokens": cached,
        "session_id": session, "workspace_id": workspace,
    }))
    .unwrap()
}

/// 【用量统计】【排行测试】所有模型调用按会话累加，没有会话身份的请求不进入排行。
/// 参数: 无
/// 返回: 无，累计值、身份或排序错误时断言失败
#[test]
fn aggregates_all_calls_before_ranking_sessions() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let records = vec![
        record(Some("a"), Some("work"), 100, 0),
        record(Some("a"), Some("work"), 200, 0),
        record(Some("b"), Some("work"), 310, 0),
        record(None, None, 1_000_000, 0),
    ];
    let (rows, total) = group_session_stats(&paths, &records, &UsageStatsQuery::default());
    assert_eq!(total, 2);
    assert_eq!(rows[0].session_id, "a");
    assert_eq!(rows[0].summary.total_requests, 2);
    assert_eq!(rows[0].summary.total_tokens, 320);
    assert_eq!(rows[0].summary.output_tokens, 20);
    assert_eq!(rows[0].summary.average_duration_ms, Some(1000.0));
    assert!(!rows[0].session_available);
}

/// 【用量统计】【排行测试】缓存折算排序与原始总量排序使用不同指标，并在排序后截取 Top。
/// 参数: 无
/// 返回: 无，排序或截取次序错误时断言失败
#[test]
fn ranks_raw_billable_and_request_totals_independently() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let records = vec![
        record(Some("cached"), Some("work"), 100_000, 99_000),
        record(Some("uncached"), Some("work"), 20_000, 0),
        record(Some("frequent"), Some("work"), 1, 0),
        record(Some("frequent"), Some("work"), 1, 0),
    ];
    for (sort, expected) in [
        ("total_tokens", "cached"),
        ("billable_tokens", "uncached"),
        ("requests", "frequent"),
    ] {
        let query = UsageStatsQuery {
            session_sort: Some(sort.into()),
            session_limit: Some(1),
            ..Default::default()
        };
        let (rows, total) = group_session_stats(&paths, &records, &query);
        assert_eq!(total, 3);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].session_id, expected);
    }
}

/// 【用量统计】【排行测试】不同项目的默认会话保持独立，旧日志不能错误归属到其中一个。
/// 参数: 无
/// 返回: 无，跨项目合并或错误定位时断言失败
#[test]
fn separates_default_sessions_and_keeps_ambiguous_legacy_usage() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    for name in ["first", "second"] {
        crate::state::ensure_workspace_session(&paths, &temp.path().join(name), "default", name)
            .unwrap();
    }
    let records = vec![
        record(Some("default"), Some("first"), 100, 0),
        record(Some("default"), Some("second"), 200, 0),
        record(Some("default"), None, 300, 0),
    ];
    let (rows, total) = group_session_stats(&paths, &records, &UsageStatsQuery::default());
    assert_eq!(total, 3);
    assert!(rows[0].workspace_id.is_none());
    assert!(rows[0].title.is_none());
    assert_eq!(rows[0].summary.total_tokens, 310);
}

/// 【用量统计】【排行测试】旧记录通过唯一会话索引恢复标题和工作区。
/// 参数: 无
/// 返回: 无，旧记录不能定位时断言失败
#[test]
fn resolves_unique_legacy_session_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    crate::state::ensure_workspace_session(
        &paths,
        &temp.path().join("project"),
        "legacy",
        "查找性能问题",
    )
    .unwrap();
    let (rows, _) = group_session_stats(
        &paths,
        &[record(Some("legacy"), None, 100, 0)],
        &UsageStatsQuery::default(),
    );
    assert_eq!(rows[0].title.as_deref(), Some("查找性能问题"));
    assert!(rows[0].workspace_id.is_some());
    assert!(rows[0].session_available);
}

/// 【用量统计】【排行测试】真实日志查询的排行使用筛选全集，不随日志翻页改变。
/// 参数: 无
/// 返回: 无，筛选或日志分页污染排行时断言失败
#[test]
fn statistics_ranking_shares_filters_but_ignores_log_pagination() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    for (session, source, tokens) in [
        ("a", "chat", 100),
        ("a", "chat", 200),
        ("b", "chat", 150),
        ("c", "compaction", 1000),
    ] {
        record_model_call(
            &paths,
            UsageRecordInput {
                provider_id: "provider",
                provider_name: "Provider",
                model: "model",
                source,
                operation: "message",
                status: "success",
                usage_source: "provider_reported",
                usage: Some(&Usage {
                    prompt_tokens: tokens,
                    total_tokens: tokens,
                    ..Default::default()
                }),
                started_at: chrono::Local::now().timestamp(),
                duration_ms: 10,
                session_id: Some(session),
                error_kind: None,
            },
        )
        .unwrap();
    }
    let stats = get_stats(
        &paths,
        UsageStatsQuery {
            range: "all".into(),
            source: Some("chat".into()),
            limit: Some(1),
            offset: Some(1),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(stats.logs.len(), 1);
    assert_eq!(stats.total_logs, 3);
    assert_eq!(stats.total_sessions, 2);
    assert_eq!(stats.session_stats[0].session_id, "a");
    assert_eq!(stats.session_stats[0].summary.total_tokens, 300);
    assert!(stats.logs[0].workspace_id.is_some());
    let serialized = serde_json::to_value(stats).unwrap();
    assert_eq!(serialized["session_stats"][0]["total_tokens"], 300);
}
