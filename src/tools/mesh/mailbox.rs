use super::address::MeshAddress;
use crate::paths::SaiPaths;
use crate::tools::subagent_state::{
    list_subagents_for_owner, queue_subagent_mesh_message, MeshMessageMeta,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// 跨进程信箱目录名（位于会话状态目录下）。
pub(crate) const INBOX_DIR: &str = "inbox";
/// 跨进程信箱文件名。
pub(crate) const INBOX_FILE: &str = "mesh.jsonl";
/// 心跳超时：写入方超过这个时长没有心跳且进程已不在，其条目判死并被清理。
pub(crate) const HEARTBEAT_TIMEOUT_MS: u64 = 60_000;
/// 单个信箱保留的最大条目数，超出的最旧条目在压缩时丢弃。
const MAX_ENTRIES: usize = 256;

/// 网格消息类型：普通消息。
pub(crate) const KIND_MESSAGE: &str = "message";
/// 网格消息类型：回复。
pub(crate) const KIND_REPLY: &str = "reply";

/// 网格消息信封。
///
/// 同一份结构既落盘（`<state_dir>/inbox/mesh.jsonl`，append-only）也驻内存。
/// `pid` + `heartbeat_at` 记录写入方的存活状态，用于清理死亡写入方留下的条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MeshEnvelope {
    /// 消息唯一 id
    pub(crate) id: String,
    /// 请求-回复关联 id；发送方生成，回复方原样带回
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) correlation_id: Option<String>,
    /// 回复应投递到的地址；缺省时回复给 `from`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) reply_to: Option<String>,
    /// 发送方地址
    pub(crate) from: String,
    /// 接收方地址
    pub(crate) to: String,
    /// `message` 或 `reply`
    #[serde(default)]
    pub(crate) kind: String,
    /// 消息正文
    pub(crate) text: String,
    /// 入队时间（Unix 毫秒）
    pub(crate) queued_at_ms: u64,
    /// 写入方进程 id
    pub(crate) pid: u32,
    /// 写入方心跳时间（Unix 毫秒）
    pub(crate) heartbeat_at: u64,
}

impl MeshEnvelope {
    /// 序列化为工具输出里的消息视图。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 消息 JSON
    pub(crate) fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "correlation_id": self.correlation_id,
            "reply_to": self.reply_to,
            "from": self.from,
            "to": self.to,
            "kind": self.kind,
            "text": self.text,
            "queued_at_ms": self.queued_at_ms,
            "pid": self.pid,
            "heartbeat_at": self.heartbeat_at,
        })
    }
}

/// 进程内信箱：状态目录 -> 消息列表。
///
/// 同进程投递优先走这里（零延迟、不落盘）；跨进程才写 `inbox/mesh.jsonl`。
fn inboxes() -> &'static Mutex<HashMap<String, Vec<MeshEnvelope>>> {
    static INBOXES: OnceLock<Mutex<HashMap<String, Vec<MeshEnvelope>>>> = OnceLock::new();
    INBOXES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 返回会话的跨进程信箱文件路径。
///
/// 参数:
/// - `state_dir`: 会话状态目录
///
/// 返回:
/// - `<state_dir>/inbox/mesh.jsonl`
pub(crate) fn inbox_file(state_dir: &Path) -> PathBuf {
    state_dir.join(INBOX_DIR).join(INBOX_FILE)
}

/// 生成一条网格消息 id。
///
/// 参数:
/// - `prefix`: id 前缀
///
/// 返回:
/// - 进程内唯一的 id
pub(crate) fn new_id(prefix: &str) -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!(
        "{prefix}-{}-{}-{}",
        unix_millis(),
        std::process::id(),
        sequence
    )
}

/// 返回当前 Unix 毫秒数。
///
/// 参数:
/// - 无
///
/// 返回:
/// - Unix 毫秒数
pub(crate) fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

/// 解析地址对应的会话状态目录。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `address`: 网格地址
///
/// 返回:
/// - 会话状态目录；广播没有单一状态目录，返回 `None`
pub(crate) fn state_dir_for(paths: &SaiPaths, address: &MeshAddress) -> Result<Option<PathBuf>> {
    match address {
        MeshAddress::Session(session_id) => {
            let (_, state_dir) = crate::state::locate_session_dirs(paths, session_id)
                .with_context(|| format!("unknown mesh session: {session_id}"))?;
            Ok(Some(state_dir))
        }
        // 子智能体地址的 owner_key 本身就是父会话状态目录
        MeshAddress::Agent { owner_key, .. } => Ok(Some(PathBuf::from(owner_key))),
        MeshAddress::Broadcast => Ok(None),
    }
}

/// 把消息投递到地址指向的信箱。
///
/// 同进程且命中子智能体表时只走内存（并直接投进子智能体的消息队列），
/// 否则落盘到目标会话的 `inbox/mesh.jsonl`，供其它进程读取。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `address`: 目标地址
/// - `envelope`: 待投递消息
///
/// 返回:
/// - 实际写入的状态目录列表
pub(crate) fn deliver(
    paths: &SaiPaths,
    address: &MeshAddress,
    envelope: &MeshEnvelope,
) -> Result<Vec<String>> {
    if matches!(address, MeshAddress::Broadcast) {
        let mut delivered = Vec::new();
        for session in crate::state::list_all_sessions(paths).unwrap_or_default() {
            deliver_to_state_dir(&session.state_dir, address, envelope)?;
            delivered.push(session.state_dir.display().to_string());
        }
        return Ok(delivered);
    }
    let state_dir = state_dir_for(paths, address)?
        .ok_or_else(|| anyhow::anyhow!("mesh address has no state dir"))?;
    deliver_to_state_dir(&state_dir, address, envelope)?;
    Ok(vec![state_dir.display().to_string()])
}

/// 把消息投递到单个会话状态目录。
///
/// 参数:
/// - `state_dir`: 目标会话状态目录
/// - `address`: 目标地址
/// - `envelope`: 待投递消息
///
/// 返回:
/// - 无
fn deliver_to_state_dir(
    state_dir: &Path,
    address: &MeshAddress,
    envelope: &MeshEnvelope,
) -> Result<()> {
    // 1. 命中本进程的子智能体表：只走内存，零延迟且不落盘
    if let (Some(agent_id), MeshAddress::Agent { owner_key, .. }) = (address.agent_id(), address) {
        if agent_is_in_process(owner_key, agent_id)
            && queue_subagent_mesh_message(
                owner_key,
                agent_id,
                &envelope.from,
                &envelope.text,
                MeshMessageMeta {
                    id: Some(envelope.id.clone()),
                    reply_to: envelope.reply_to.clone(),
                    from_addr: Some(envelope.from.clone()),
                },
            )
            .is_ok()
        {
            remember(state_dir, envelope.clone());
            return Ok(());
        }
        // 子智能体已进入终态：退回落盘，消息仍然送达信箱
    }
    // 2. 其它情况落盘，其它进程能直接读到
    append_to_disk(state_dir, envelope)
}

/// 判断子智能体是否在本进程的子智能体表里。
///
/// 参数:
/// - `owner_key`: 父会话状态目录
/// - `agent_id`: 子智能体 id
///
/// 返回:
/// - 在本进程内为 true
fn agent_is_in_process(owner_key: &str, agent_id: &str) -> bool {
    list_subagents_for_owner(owner_key)
        .iter()
        .any(|snapshot| snapshot.id == agent_id)
}

/// 在进程内信箱里留一份消息。
///
/// 参数:
/// - `state_dir`: 目标会话状态目录
/// - `envelope`: 已投递消息
///
/// 返回:
/// - 无
fn remember(state_dir: &Path, envelope: MeshEnvelope) {
    let key = state_dir.display().to_string();
    if let Ok(mut inboxes) = inboxes().lock() {
        inboxes.entry(key).or_default().push(envelope);
    }
}

/// 追加一条消息到跨进程信箱文件。
///
/// 参数:
/// - `state_dir`: 目标会话状态目录
/// - `envelope`: 待写入消息
///
/// 返回:
/// - 无
fn append_to_disk(state_dir: &Path, envelope: &MeshEnvelope) -> Result<()> {
    let path = inbox_file(state_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let encoded = serde_json::to_string(envelope).context("failed to encode mesh message")?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    writeln!(file, "{encoded}").with_context(|| format!("failed to append {}", path.display()))?;
    file.flush().ok();
    compact_if_needed(&path)
}

/// 列出信箱里的全部消息（内存 + 磁盘，按入队时间正序）。
///
/// 参数:
/// - `state_dir`: 会话状态目录
///
/// 返回:
/// - 消息列表；按 `queued_at_ms` 升序，同毫秒按 id 升序
pub(crate) fn list(state_dir: &Path) -> Vec<MeshEnvelope> {
    let key = state_dir.display().to_string();
    let mut messages = match inboxes().lock() {
        Ok(inboxes) => inboxes.get(&key).cloned().unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    let now = unix_millis();
    messages.extend(
        read_disk(state_dir)
            .into_iter()
            .filter(|envelope| !writer_is_dead(envelope, now)),
    );
    // 同一条消息可能既在内存又在磁盘（例如本进程投递后压缩重写），按 id 去重
    let mut seen = std::collections::HashSet::new();
    messages.retain(|envelope| seen.insert(envelope.id.clone()));
    messages.sort_by(|left, right| {
        left.queued_at_ms
            .cmp(&right.queued_at_ms)
            .then_with(|| left.id.cmp(&right.id))
    });
    messages
}

/// 按关联 id 查找一条普通消息。
///
/// 供 `mesh_reply` 找到自己收到的原始消息，从而拿到回复地址；找不到就不能回复，
/// 也就无法伪造别人的关联 id。
///
/// 参数:
/// - `state_dir`: 会话状态目录
/// - `correlation_id`: 关联 id
///
/// 返回:
/// - 自己的信箱里最先匹配的消息；没有则 `None`
pub(crate) fn find(state_dir: &Path, correlation_id: &str) -> Option<MeshEnvelope> {
    list(state_dir).into_iter().find(|envelope| {
        envelope.kind != KIND_REPLY && envelope.correlation_id.as_deref() == Some(correlation_id)
    })
}

/// 按关联 id 查找一条回复。
///
/// 参数:
/// - `state_dir`: 会话状态目录
/// - `correlation_id`: 关联 id
///
/// 返回:
/// - 匹配的回复；尚未收到则 `None`
pub(crate) fn find_reply(state_dir: &Path, correlation_id: &str) -> Option<MeshEnvelope> {
    list(state_dir).into_iter().find(|envelope| {
        envelope.kind == KIND_REPLY && envelope.correlation_id.as_deref() == Some(correlation_id)
    })
}

/// 读取磁盘信箱里的全部条目，坏行跳过。
///
/// 参数:
/// - `state_dir`: 会话状态目录
///
/// 返回:
/// - 解析成功的消息列表
fn read_disk(state_dir: &Path) -> Vec<MeshEnvelope> {
    let path = inbox_file(state_dir);
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<MeshEnvelope>(line).ok())
        .collect()
}

/// 判定写入方是否已失联。
///
/// 心跳超过 60s 且进程已不在，说明写入方写完就死了，其条目不再投递。
///
/// 参数:
/// - `envelope`: 待判定消息
/// - `now_ms`: 当前 Unix 毫秒
///
/// 返回:
/// - 写入方已失联时返回 true
pub(crate) fn writer_is_dead(envelope: &MeshEnvelope, now_ms: u64) -> bool {
    now_ms.saturating_sub(envelope.heartbeat_at) > HEARTBEAT_TIMEOUT_MS
        && !process_alive(envelope.pid)
}

/// 判断进程是否还在。
///
/// 参数:
/// - `pid`: 进程 id
///
/// 返回:
/// - 进程还在为 true；非 Linux 平台无法可靠探测时按存活处理
fn process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    if pid == std::process::id() {
        return true;
    }
    #[cfg(target_os = "linux")]
    {
        PathBuf::from(format!("/proc/{pid}")).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

/// 必要时压缩信箱：清理死亡写入方的条目，并把条数压回上限。
///
/// 参数:
/// - `path`: 信箱文件
///
/// 返回:
/// - 无
fn compact_if_needed(path: &Path) -> Result<()> {
    let entries = read_file(path);
    let now = unix_millis();
    let alive = entries
        .iter()
        .filter(|envelope| !writer_is_dead(envelope, now))
        .cloned()
        .collect::<Vec<_>>();
    if alive.len() == entries.len() && alive.len() <= MAX_ENTRIES {
        return Ok(());
    }
    let start = alive.len().saturating_sub(MAX_ENTRIES);
    rewrite_file(path, &alive[start..])
}

/// 读取信箱文件的全部条目。
///
/// 参数:
/// - `path`: 信箱文件
///
/// 返回:
/// - 解析成功的消息列表
fn read_file(path: &Path) -> Vec<MeshEnvelope> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<MeshEnvelope>(line).ok())
        .collect()
}

/// 原子重写信箱文件。
///
/// 先写临时文件再 rename，避免读取方读到写了一半的文件。
///
/// 参数:
/// - `path`: 信箱文件
/// - `entries`: 保留的条目
///
/// 返回:
/// - 无
fn rewrite_file(path: &Path, entries: &[MeshEnvelope]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create {}", parent.display()))?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temp file in {}", parent.display()))?;
    for envelope in entries {
        let encoded = serde_json::to_string(envelope).context("failed to encode mesh message")?;
        writeln!(temp, "{encoded}").context("failed to write mesh message")?;
    }
    temp.flush().context("failed to flush mesh inbox")?;
    temp.persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}
