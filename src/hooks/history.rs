//! 有界执行记录和大输出留档，模型只接收裁剪正文及回读路径。
use super::{decisions::HookOutcome, HookContext, HookEvent};
use crate::{config::HookItem, paths::SaiPaths};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::Path,
    sync::{Mutex, OnceLock},
    time::Instant,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct HookRecord {
    pub name: String,
    pub event: String,
    pub session_id: String,
    pub source: String,
    pub status: String,
    pub status_message: Option<String>,
    pub timestamp: i64,
    pub duration_ms: u64,
    pub error: Option<String>,
    pub output: Vec<String>,
}

/// 【Hooks】【记录同步】无参数，返回保护本进程 JSONL 追加和轮换的锁
fn lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(Default::default)
}

/// 【Hooks】【执行记录】参数为条目、事件、上下文、状态、开始时间和结果，返回无值，保存失败只记录错误
pub(super) fn record(
    hook: &HookItem,
    event: HookEvent,
    context: &HookContext,
    status: &str,
    started: Instant,
    result: Option<&Result<HookOutcome>>,
) {
    let Some(source) = &hook.options.runtime else {
        return;
    };
    let record = HookRecord {
        name: hook.name.clone(),
        event: event.as_str().into(),
        session_id: context.session_id.clone(),
        source: source
            .path
            .as_ref()
            .map(|file| file.display().to_string())
            .unwrap_or_else(|| "config.jsonc".into()),
        status: status.into(),
        status_message: hook.options.status_message.clone(),
        timestamp: chrono::Utc::now().timestamp_millis(),
        duration_ms: started.elapsed().as_millis() as u64,
        error: result
            .and_then(|result| result.as_ref().err())
            .map(|error| format!("{error:#}").chars().take(2048).collect()),
        output: result
            .and_then(|result| result.as_ref().ok())
            .map(|result| {
                result
                    .context
                    .iter()
                    .chain(&result.messages)
                    .map(|text| text.chars().take(2048).collect())
                    .collect()
            })
            .unwrap_or_default(),
    };
    let saved = (|| -> Result<()> {
        let _lock = lock().lock().unwrap_or_else(|error| error.into_inner());
        std::fs::create_dir_all(&source.history_dir)?;
        let file = source.history_dir.join("events.jsonl");
        if std::fs::metadata(&file).is_ok_and(|metadata| metadata.len() > 4_194_304) {
            let previous = source.history_dir.join("events.previous.jsonl");
            if previous.exists() {
                std::fs::remove_file(&previous)?;
            }
            std::fs::rename(&file, previous)?;
        }
        let mut options = std::fs::OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(file)?;
        serde_json::to_writer(&mut file, &record)?;
        file.write_all(b"\n")?;
        Ok(())
    })();
    if let Err(error) = saved {
        eprintln!("【Hooks】【记录失败】{error:#}");
    }
}

/// 【Hooks】【记录查询】参数为应用路径与可选会话，返回最近一百条记录，不读取大输出正文
pub(crate) fn recent(paths: &SaiPaths, session: Option<&str>) -> Vec<HookRecord> {
    let _lock = lock().lock().unwrap_or_else(|error| error.into_inner());
    let text = super::source_io::read(&paths.data_dir.join("hook-history/events.jsonl"), 5_242_880)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_default();
    text.lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<HookRecord>(line).ok())
        .filter(|record| session.is_none_or(|id| record.session_id == id))
        .take(100)
        .collect()
}

/// 【Hooks】【输出预算】参数为结果和动作，返回无值，按每项约定预算裁剪并尽量保存正文
pub(super) fn bound(result: &mut HookOutcome, hook: &HookItem) {
    let budget = hook.options.output_tokens.unwrap_or(2500).min(16_000);
    for text in result
        .context
        .iter_mut()
        .chain(&mut result.messages)
        .chain(result.deny.iter_mut())
    {
        if crate::token_estimate::estimate_tokens(text) <= budget.max(1) {
            continue;
        }
        let pointer = if budget > 0 {
            hook.options
                .runtime
                .as_ref()
                .and_then(|source| save_output(&source.output_dir, text).ok())
        } else {
            None
        };
        let chars = text.chars().collect::<Vec<_>>();
        let mut end = chars.len();
        let mut start = 0;
        while start < end {
            let middle = (start + end + 1) / 2;
            let candidate: String = chars
                .iter()
                .take(middle / 2)
                .chain(chars.iter().skip(chars.len().saturating_sub(middle / 2)))
                .collect();
            if crate::token_estimate::estimate_tokens(&candidate) <= budget.max(1) {
                start = middle;
            } else {
                end = middle - 1;
            }
        }
        *text = format!(
            "{}\n[hook output truncated]\n{}{}",
            chars[..start / 2].iter().collect::<String>(),
            chars[chars.len().saturating_sub(start / 2)..]
                .iter()
                .collect::<String>(),
            pointer
                .map(|file| format!("\nFull hook output: {}", file.display()))
                .unwrap_or_default()
        );
    }
}

/// 【Hooks】【正文留档】参数为私有输出目录与正文，返回持久化文件路径，内容不进入执行日志
fn save_output(root: &Path, text: &str) -> Result<std::path::PathBuf> {
    std::fs::create_dir_all(root)?;
    let mut file = tempfile::Builder::new()
        .prefix("hook-")
        .suffix(".txt")
        .tempfile_in(root)?;
    file.write_all(text.as_bytes())?;
    let (_, file) = file.keep()?;
    Ok(file)
}
