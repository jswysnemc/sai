use crate::llm::Usage;
use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use chrono::{Datelike, Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const USAGE_DIR_NAME: &str = "usage";

/// 单次模型调用用量记录。
///
/// `input_tokens` 是输入侧总量（含命中缓存的部分），`cache_read_tokens` 与
/// `cache_write_tokens` 是其中的缓存构成。供应商对缓存另有计价系数，
/// 只看总量会与账单出现数量级差异，因此明细必须落盘保留。
/// 两个缓存字段对历史日志缺省为 None，反序列化旧记录不受影响。
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
    /// `input_tokens` 中命中缓存读取的部分
    #[serde(default)]
    pub cache_read_tokens: Option<u64>,
    /// `input_tokens` 中写入缓存的部分
    #[serde(default)]
    pub cache_write_tokens: Option<u64>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub error_kind: Option<String>,
}

impl UsageRecord {
    /// 返回本条记录的总令牌数。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 上报的总量；缺失时用输入与输出之和补齐
    pub fn total_tokens_or_sum(&self) -> u64 {
        self.total_tokens.unwrap_or_else(|| {
            self.input_tokens
                .unwrap_or(0)
                .saturating_add(self.output_tokens.unwrap_or(0))
        })
    }

    /// 返回本条记录命中缓存读取的令牌数。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 缓存读取量；历史记录无该字段时为 0
    pub fn cache_read(&self) -> u64 {
        self.cache_read_tokens.unwrap_or(0)
    }

    /// 返回本条记录写入缓存的令牌数。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 缓存写入量；历史记录无该字段时为 0
    pub fn cache_write(&self) -> u64 {
        self.cache_write_tokens.unwrap_or(0)
    }
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
    // 1. 拆解用量：无上报时全部记为 None，并把来源标记为 missing
    let usage_fields = match input.usage {
        Some(usage) => UsageFields {
            input_tokens: Some(usage.prompt_tokens),
            output_tokens: Some(usage.completion_tokens),
            total_tokens: Some(usage.total_tokens),
            cache_read_tokens: Some(usage.cache_read_tokens),
            cache_write_tokens: Some(usage.cache_write_tokens),
            usage_source: if input.usage_source.trim().is_empty() {
                "provider_reported".to_string()
            } else {
                input.usage_source.to_string()
            },
        },
        None => UsageFields {
            usage_source: "missing".to_string(),
            ..UsageFields::default()
        },
    };
    // 2. 组装记录并按月份追加落盘
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
        usage_source: usage_fields.usage_source,
        input_tokens: usage_fields.input_tokens,
        output_tokens: usage_fields.output_tokens,
        total_tokens: usage_fields.total_tokens,
        cache_read_tokens: usage_fields.cache_read_tokens,
        cache_write_tokens: usage_fields.cache_write_tokens,
        session_id: input.session_id.map(str::to_string),
        error_kind: input.error_kind.map(str::to_string),
    };
    append_record(&usage_dir(paths), &record)
}

/// 从用量结构拆出的落盘字段，仅用于组装记录时传值。
#[derive(Default)]
struct UsageFields {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    total_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
    usage_source: String,
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

/// 读取目录下的全部记录。
///
/// 参数:
/// - `dir`: 用量日志目录
/// - `start`: 时间下界，用于跳过整月过旧的文件
///
/// 返回:
/// - 记录列表与解析失败的行数
pub(crate) fn read_records(dir: &Path, start: Option<i64>) -> Result<(Vec<UsageRecord>, usize)> {
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
