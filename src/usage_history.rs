use crate::llm::Usage;
use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use chrono::{Datelike, Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const USAGE_DIR_NAME: &str = "usage";
const DEFAULT_LOG_LIMIT: usize = 100;
const MAX_LOG_LIMIT: usize = 500;

/// 单次模型调用用量记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub id: String,
    pub created_at: i64,
    pub completed_at: i64,
    pub duration_ms: u64,
    pub source: String,
    pub operation: String,
    pub provider_id: String,
    pub provider_name: String,
    pub model: String,
    pub status: String,
    pub usage_source: String,
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub output_tokens: Option<u64>,
    #[serde(default)]
    pub total_tokens: Option<u64>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub error_kind: Option<String>,
}

/// 写入用量日志的输入参数。
pub struct UsageRecordInput<'a> {
    pub provider_id: &'a str,
    pub provider_name: &'a str,
    pub model: &'a str,
    pub source: &'a str,
    pub operation: &'a str,
    pub status: &'a str,
    pub usage: Option<&'a Usage>,
    pub usage_source: &'a str,
    pub started_at: i64,
    pub duration_ms: u64,
    pub session_id: Option<&'a str>,
    pub error_kind: Option<&'a str>,
}

/// 统计查询参数。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct UsageStatsQuery {
    #[serde(default = "default_range")]
    pub range: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub provider_search: Option<String>,
    #[serde(default)]
    pub model_search: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

fn default_range() -> String {
    "7d".to_string()
}

/// 汇总卡片数据。
#[derive(Debug, Clone, Default, Serialize)]
pub struct UsageSummary {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub missing_usage_requests: u64,
    pub provider_reported_requests: u64,
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub average_duration_ms: Option<f64>,
}

/// 趋势图按日点。
#[derive(Debug, Clone, Default, Serialize)]
pub struct UsageTrendPoint {
    pub date: String,
    pub label: String,
    pub requests: u64,
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Provider / Model 分组统计。
#[derive(Debug, Clone, Default, Serialize)]
pub struct UsageGroupStats {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub request_count: u64,
    pub success_count: u64,
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub average_duration_ms: Option<f64>,
    pub last_used_at: Option<i64>,
}

/// 用量统计 API 响应。
#[derive(Debug, Clone, Serialize)]
pub struct UsageStatsResponse {
    pub summary: UsageSummary,
    pub trend: Vec<UsageTrendPoint>,
    pub logs: Vec<UsageRecord>,
    pub provider_stats: Vec<UsageGroupStats>,
    pub model_stats: Vec<UsageGroupStats>,
    pub total_logs: usize,
    pub skipped_records: usize,
}

/// 返回全局用量日志目录。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 用量日志目录
pub fn usage_dir(paths: &SaiPaths) -> PathBuf {
    paths.data_dir.join(USAGE_DIR_NAME)
}

/// 追加一次模型调用记录。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `input`: 调用元数据与用量
///
/// 返回:
/// - 写入是否成功
pub fn record_model_call(paths: &SaiPaths, input: UsageRecordInput<'_>) -> Result<()> {
    let completed_at = Local::now().timestamp();
    let (input_tokens, output_tokens, total_tokens, usage_source) = match input.usage {
        Some(usage) => (
            Some(usage.prompt_tokens),
            Some(usage.completion_tokens),
            Some(usage.total_tokens),
            if input.usage_source.trim().is_empty() {
                "provider_reported".to_string()
            } else {
                input.usage_source.to_string()
            },
        ),
        None => (None, None, None, "missing".to_string()),
    };
    let record = UsageRecord {
        id: format!("usage_{}", uuid::Uuid::new_v4()),
        created_at: input.started_at,
        completed_at,
        duration_ms: input.duration_ms,
        source: input.source.to_string(),
        operation: input.operation.to_string(),
        provider_id: input.provider_id.to_string(),
        provider_name: input.provider_name.to_string(),
        model: input.model.to_string(),
        status: input.status.to_string(),
        usage_source,
        input_tokens,
        output_tokens,
        total_tokens,
        session_id: input.session_id.map(str::to_string),
        error_kind: input.error_kind.map(str::to_string),
    };
    append_record(&usage_dir(paths), &record)
}

/// 查询聚合统计。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `query`: 筛选与分页
///
/// 返回:
/// - 汇总、趋势、日志与分组
pub fn get_stats(paths: &SaiPaths, query: UsageStatsQuery) -> Result<UsageStatsResponse> {
    let start = range_start(&query.range);
    let (records, skipped_records) = read_records(&usage_dir(paths), start)?;
    let filtered = filter_records(records, &query);
    let total_logs = filtered.len();
    let summary = summarize(&filtered);
    let trend = build_trend(&filtered, &query.range);
    let provider_stats = group_provider_stats(&filtered);
    let model_stats = group_model_stats(&filtered);
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(DEFAULT_LOG_LIMIT).min(MAX_LOG_LIMIT);
    let logs = filtered.into_iter().skip(offset).take(limit).collect();
    Ok(UsageStatsResponse {
        summary,
        trend,
        logs,
        provider_stats,
        model_stats,
        total_logs,
        skipped_records,
    })
}

/// 清空全部用量日志文件。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 清除是否成功
pub fn clear_all(paths: &SaiPaths) -> Result<()> {
    let dir = usage_dir(paths);
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(&dir).with_context(|| format!("read usage dir {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
            fs::remove_file(&path)
                .with_context(|| format!("remove usage file {}", path.display()))?;
        }
    }
    Ok(())
}

/// 追加一行 JSONL。
fn append_record(dir: &Path, record: &UsageRecord) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("create usage dir {}", dir.display()))?;
    let path = dir.join(monthly_file_name(record.created_at));
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("open usage file {}", path.display()))?;
    let line = serde_json::to_string(record).context("serialize usage record")?;
    writeln!(file, "{line}").with_context(|| format!("write usage file {}", path.display()))?;
    Ok(())
}

fn monthly_file_name(timestamp: i64) -> String {
    let date = Local
        .timestamp_opt(timestamp, 0)
        .single()
        .unwrap_or_else(Local::now);
    format!("usage-{:04}-{:02}.jsonl", date.year(), date.month())
}

fn read_records(dir: &Path, start: Option<i64>) -> Result<(Vec<UsageRecord>, usize)> {
    if !dir.exists() {
        return Ok((Vec::new(), 0));
    }
    let mut records = Vec::new();
    let mut skipped = 0usize;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        if usage_file_is_before_start(&path, start) {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            skipped = skipped.saturating_add(1);
            continue;
        };
        for line in content.lines().filter(|line| !line.trim().is_empty()) {
            match serde_json::from_str::<UsageRecord>(line) {
                Ok(record) => records.push(record),
                Err(_) => skipped = skipped.saturating_add(1),
            }
        }
    }
    Ok((records, skipped))
}

fn usage_file_is_before_start(path: &Path, start: Option<i64>) -> bool {
    let Some(start) = start else {
        return false;
    };
    let Some(next_month_start) = usage_file_next_month_start(path) else {
        return false;
    };
    next_month_start <= start
}

fn usage_file_next_month_start(path: &Path) -> Option<i64> {
    let file_name = path.file_name()?.to_str()?;
    let stem = file_name.strip_prefix("usage-")?.strip_suffix(".jsonl")?;
    let (year, month) = stem.split_once('-')?;
    let mut year = year.parse::<i32>().ok()?;
    let mut month = month.parse::<u32>().ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }
    if month == 12 {
        year = year.saturating_add(1);
        month = 1;
    } else {
        month += 1;
    }
    Local
        .with_ymd_and_hms(year, month, 1, 0, 0, 0)
        .single()
        .map(|date| date.timestamp())
}

fn filter_records(mut records: Vec<UsageRecord>, query: &UsageStatsQuery) -> Vec<UsageRecord> {
    let start = range_start(&query.range);
    let source = normalized_filter(query.source.as_deref());
    let status = normalized_filter(query.status.as_deref());
    let provider_search = normalized_search(query.provider_search.as_deref());
    let model_search = normalized_search(query.model_search.as_deref());
    records.retain(|record| {
        if let Some(start) = start {
            if record.created_at < start {
                return false;
            }
        }
        if let Some(source) = source.as_deref() {
            if record.source != source {
                return false;
            }
        }
        if let Some(status) = status.as_deref() {
            if status == "missing_usage" {
                if record.usage_source != "missing" {
                    return false;
                }
            } else if record.status != status {
                return false;
            }
        }
        if let Some(search) = provider_search.as_deref() {
            let haystack =
                format!("{} {}", record.provider_id, record.provider_name).to_ascii_lowercase();
            if !haystack.contains(search) {
                return false;
            }
        }
        if let Some(search) = model_search.as_deref() {
            if !record.model.to_ascii_lowercase().contains(search) {
                return false;
            }
        }
        true
    });
    records.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    records
}

fn range_start(range: &str) -> Option<i64> {
    let now = Local::now().timestamp();
    match range {
        "today" => Local::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .and_then(|naive| Local.from_local_datetime(&naive).single())
            .map(|dt| dt.timestamp()),
        "1d" => Some(now.saturating_sub(86_400)),
        "30d" => Some(now.saturating_sub(30 * 86_400)),
        "90d" => Some(now.saturating_sub(90 * 86_400)),
        "all" => None,
        _ => Some(now.saturating_sub(7 * 86_400)),
    }
}

fn normalized_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "all")
        .map(str::to_string)
}

fn normalized_search(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn summarize(records: &[UsageRecord]) -> UsageSummary {
    let mut summary = UsageSummary::default();
    let mut duration_total = 0u64;
    let mut duration_count = 0u64;
    for record in records {
        summary.total_requests = summary.total_requests.saturating_add(1);
        if record.status == "success" {
            summary.successful_requests = summary.successful_requests.saturating_add(1);
        } else {
            summary.failed_requests = summary.failed_requests.saturating_add(1);
        }
        if record.usage_source == "missing" {
            summary.missing_usage_requests = summary.missing_usage_requests.saturating_add(1);
        }
        if record.usage_source == "provider_reported" {
            summary.provider_reported_requests =
                summary.provider_reported_requests.saturating_add(1);
        }
        summary.total_tokens = summary
            .total_tokens
            .saturating_add(record_total_tokens(record));
        summary.input_tokens = summary
            .input_tokens
            .saturating_add(record.input_tokens.unwrap_or(0));
        summary.output_tokens = summary
            .output_tokens
            .saturating_add(record.output_tokens.unwrap_or(0));
        duration_total = duration_total.saturating_add(record.duration_ms);
        duration_count = duration_count.saturating_add(1);
    }
    if duration_count > 0 {
        summary.average_duration_ms = Some(duration_total as f64 / duration_count as f64);
    }
    summary
}

fn record_total_tokens(record: &UsageRecord) -> u64 {
    record
        .total_tokens
        .unwrap_or_else(|| {
            record
                .input_tokens
                .unwrap_or(0)
                .saturating_add(record.output_tokens.unwrap_or(0))
        })
}

fn build_trend(records: &[UsageRecord], range: &str) -> Vec<UsageTrendPoint> {
    let mut by_day: BTreeMap<String, UsageTrendPoint> = BTreeMap::new();
    for record in records {
        let date = Local
            .timestamp_opt(record.created_at, 0)
            .single()
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let label = Local
            .timestamp_opt(record.created_at, 0)
            .single()
            .map(|dt| dt.format("%m-%d").to_string())
            .unwrap_or_else(|| date.clone());
        let point = by_day.entry(date.clone()).or_insert_with(|| UsageTrendPoint {
            date: date.clone(),
            label,
            ..UsageTrendPoint::default()
        });
        point.requests = point.requests.saturating_add(1);
        point.total_tokens = point
            .total_tokens
            .saturating_add(record_total_tokens(record));
        point.input_tokens = point
            .input_tokens
            .saturating_add(record.input_tokens.unwrap_or(0));
        point.output_tokens = point
            .output_tokens
            .saturating_add(record.output_tokens.unwrap_or(0));
    }
    // 1 天内按小时聚合可读性更好；其它按日
    if range == "1d" || range == "today" {
        return build_hourly_trend(records);
    }
    by_day.into_values().collect()
}

fn build_hourly_trend(records: &[UsageRecord]) -> Vec<UsageTrendPoint> {
    let mut by_hour: BTreeMap<String, UsageTrendPoint> = BTreeMap::new();
    for record in records {
        let Some(dt) = Local.timestamp_opt(record.created_at, 0).single() else {
            continue;
        };
        let date = dt.format("%Y-%m-%d %H:00").to_string();
        let label = dt.format("%H:00").to_string();
        let point = by_hour.entry(date.clone()).or_insert_with(|| UsageTrendPoint {
            date: date.clone(),
            label,
            ..UsageTrendPoint::default()
        });
        point.requests = point.requests.saturating_add(1);
        point.total_tokens = point
            .total_tokens
            .saturating_add(record_total_tokens(record));
        point.input_tokens = point
            .input_tokens
            .saturating_add(record.input_tokens.unwrap_or(0));
        point.output_tokens = point
            .output_tokens
            .saturating_add(record.output_tokens.unwrap_or(0));
    }
    by_hour.into_values().collect()
}

#[derive(Default)]
struct GroupAcc {
    id: String,
    label: String,
    provider_id: Option<String>,
    provider_name: Option<String>,
    model: Option<String>,
    request_count: u64,
    success_count: u64,
    total_tokens: u64,
    input_tokens: u64,
    output_tokens: u64,
    duration_total: u64,
    last_used_at: Option<i64>,
}

impl GroupAcc {
    fn finish(self) -> UsageGroupStats {
        UsageGroupStats {
            id: self.id,
            label: self.label,
            provider_id: self.provider_id,
            provider_name: self.provider_name,
            model: self.model,
            request_count: self.request_count,
            success_count: self.success_count,
            total_tokens: self.total_tokens,
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            average_duration_ms: if self.request_count > 0 {
                Some(self.duration_total as f64 / self.request_count as f64)
            } else {
                None
            },
            last_used_at: self.last_used_at,
        }
    }
}

fn group_provider_stats(records: &[UsageRecord]) -> Vec<UsageGroupStats> {
    let mut map: BTreeMap<String, GroupAcc> = BTreeMap::new();
    for record in records {
        let key = record.provider_id.clone();
        let acc = map.entry(key.clone()).or_insert_with(|| GroupAcc {
            id: key.clone(),
            label: if record.provider_name.trim().is_empty() {
                record.provider_id.clone()
            } else {
                record.provider_name.clone()
            },
            provider_id: Some(record.provider_id.clone()),
            provider_name: Some(record.provider_name.clone()),
            model: None,
            ..GroupAcc::default()
        });
        accumulate(acc, record);
    }
    finish_groups(map)
}

fn group_model_stats(records: &[UsageRecord]) -> Vec<UsageGroupStats> {
    let mut map: BTreeMap<String, GroupAcc> = BTreeMap::new();
    for record in records {
        let key = format!("{}::{}", record.provider_id, record.model);
        let acc = map.entry(key.clone()).or_insert_with(|| GroupAcc {
            id: key.clone(),
            label: record.model.clone(),
            provider_id: Some(record.provider_id.clone()),
            provider_name: Some(record.provider_name.clone()),
            model: Some(record.model.clone()),
            ..GroupAcc::default()
        });
        accumulate(acc, record);
    }
    finish_groups(map)
}

fn accumulate(acc: &mut GroupAcc, record: &UsageRecord) {
    acc.request_count = acc.request_count.saturating_add(1);
    if record.status == "success" {
        acc.success_count = acc.success_count.saturating_add(1);
    }
    acc.total_tokens = acc.total_tokens.saturating_add(record_total_tokens(record));
    acc.input_tokens = acc
        .input_tokens
        .saturating_add(record.input_tokens.unwrap_or(0));
    acc.output_tokens = acc
        .output_tokens
        .saturating_add(record.output_tokens.unwrap_or(0));
    acc.duration_total = acc.duration_total.saturating_add(record.duration_ms);
    acc.last_used_at = Some(
        acc.last_used_at
            .unwrap_or(0)
            .max(record.created_at),
    );
}

fn finish_groups(map: BTreeMap<String, GroupAcc>) -> Vec<UsageGroupStats> {
    let mut rows: Vec<UsageGroupStats> = map.into_values().map(GroupAcc::finish).collect();
    rows.sort_by(|a, b| b.total_tokens.cmp(&a.total_tokens));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::SaiPaths;
    use std::path::PathBuf;

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

    #[test]
    fn records_and_aggregates_usage() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let usage = Usage {
            prompt_tokens: 100,
            completion_tokens: 20,
            total_tokens: 120,
        };
        let started = Local::now().timestamp();
        record_model_call(
            &paths,
            UsageRecordInput {
                provider_id: "p1",
                provider_name: "Provider One",
                model: "model-a",
                source: "chat",
                operation: "turn",
                status: "success",
                usage: Some(&usage),
                usage_source: "provider_reported",
                started_at: started,
                duration_ms: 1500,
                session_id: Some("sess-1"),
                error_kind: None,
            },
        )
        .unwrap();
        record_model_call(
            &paths,
            UsageRecordInput {
                provider_id: "p1",
                provider_name: "Provider One",
                model: "model-b",
                source: "compaction",
                operation: "summary",
                status: "success",
                usage: Some(&Usage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                }),
                usage_source: "provider_reported",
                started_at: started,
                duration_ms: 200,
                session_id: Some("sess-1"),
                error_kind: None,
            },
        )
        .unwrap();

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
        let _ = PathBuf::new();
    }
}
