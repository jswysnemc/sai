//! 模型调用用量的落盘、查询与统计。
//!
//! 模块按职责拆分：`record` 负责记录结构与 JSONL 读写，`query` 负责筛选，
//! `billing` 负责缓存计价折算，`summary` / `trend` / `grouping` 负责三种维度的聚合，
//! `stats` 负责编排出对外响应。

mod billing;
mod grouping;
mod query;
mod record;
mod stats;
mod summary;
mod trend;

pub use query::UsageStatsQuery;
pub use record::{clear_all, record_model_call, UsageRecordInput};
pub use stats::{get_stats, UsageStatsResponse};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::Usage;
    use crate::paths::SaiPaths;
    use chrono::Local;
    use std::path::Path;

    fn test_paths(root: &Path) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.jsonc"),
            skills_dir: root.join("config/skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish"),
            bash_hook_file: root.join("bash"),
            zsh_hook_file: root.join("zsh"),
            powershell_hook_file: root.join("ps1"),
        }
    }

    fn record_call(paths: &SaiPaths, model: &str, source: &str, usage: &Usage, started: i64) {
        record_model_call(
            paths,
            UsageRecordInput {
                provider_id: "p1",
                provider_name: "Provider One",
                model,
                source,
                operation: "turn",
                status: "success",
                usage: Some(usage),
                usage_source: "provider_reported",
                started_at: started,
                duration_ms: 500,
                session_id: Some("sess-1"),
                error_kind: None,
            },
        )
        .unwrap();
    }

    /// 验证记录落盘后可按各维度聚合，且清空后归零。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无；断言失败则测试不通过
    #[test]
    fn records_and_aggregates_usage() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let started = Local::now().timestamp();
        record_call(
            &paths,
            "model-a",
            "chat",
            &Usage {
                prompt_tokens: 100,
                completion_tokens: 20,
                total_tokens: 120,
                ..Usage::default()
            },
            started,
        );
        record_call(
            &paths,
            "model-b",
            "compaction",
            &Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                ..Usage::default()
            },
            started,
        );

        let stats = get_stats(
            &paths,
            UsageStatsQuery {
                range: "all".to_string(),
                ..UsageStatsQuery::default()
            },
        )
        .unwrap();
        assert_eq!(stats.summary.total_requests, 2);
        assert_eq!(stats.summary.total_tokens, 135);
        assert_eq!(stats.provider_stats.len(), 1);
        assert_eq!(stats.model_stats.len(), 2);
        assert_eq!(stats.total_logs, 2);

        clear_all(&paths).unwrap();
        let empty = get_stats(
            &paths,
            UsageStatsQuery {
                range: "all".to_string(),
                ..UsageStatsQuery::default()
            },
        )
        .unwrap();
        assert_eq!(empty.summary.total_requests, 0);
    }

    /// 验证缓存密集的调用在计费口径下远低于原始输入量。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无；断言失败则测试不通过
    #[test]
    fn separates_billable_input_from_raw_input() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        record_call(
            &paths,
            "claude-opus",
            "chat",
            &Usage {
                prompt_tokens: 100_000,
                completion_tokens: 1_000,
                total_tokens: 101_000,
                cache_read_tokens: 95_000,
                cache_write_tokens: 0,
            },
            Local::now().timestamp(),
        );
        let stats = get_stats(
            &paths,
            UsageStatsQuery {
                range: "all".to_string(),
                ..UsageStatsQuery::default()
            },
        )
        .unwrap();
        assert_eq!(stats.summary.input_tokens, 100_000);
        assert_eq!(stats.summary.cache_read_tokens, 95_000);
        // 5000 原价 + 95000 * 0.1 = 14500
        assert_eq!(stats.summary.billable_input_tokens, 14_500);
        assert_eq!(stats.summary.billable_total_tokens, 15_500);
    }
}
