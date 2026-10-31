use crate::{catalog::CatalogEntry, store::Store};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, Write};

pub const WORKER_ARG: &str = "--internal-codemode-worker";
pub const MAX_FRAME_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_BRIDGE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_CODE_BYTES: usize = 1024 * 1024;
pub const MAX_CALLS: usize = 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionRequest {
    pub code: String,
    pub timeout_ms: u64,
    pub catalog: Vec<CatalogEntry>,
    pub store: Store,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkerEvent {
    Call {
        id: u64,
        name: String,
        arguments: Value,
    },
    Output {
        kind: OutputKind,
        value: String,
    },
    Done {
        error: Option<String>,
        writes: Store,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    Text,
    Console,
    Image,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolReply {
    pub id: u64,
    pub value: Value,
    pub error: Option<String>,
}

/// 【CodeMode】【协议读取】从 reader 读取一条有容量上限的 JSON 帧，返回解码值。
pub fn read_frame<T: serde::de::DeserializeOwned>(reader: &mut impl BufRead) -> Result<T> {
    let mut bytes = Vec::new();
    let mut bounded = std::io::Read::take(reader, (MAX_FRAME_BYTES + 1) as u64);
    bounded.read_until(b'\n', &mut bytes)?;
    if bytes.len() > MAX_FRAME_BYTES {
        bail!("codemode IPC frame exceeds size limit");
    }
    if bytes.last() != Some(&b'\n') {
        bail!("codemode worker channel closed or incomplete frame");
    }
    Ok(serde_json::from_slice(&bytes)?)
}

/// 【CodeMode】【协议写入】把 value 编码到 writer 并刷新，返回写入结果。
pub fn write_frame(writer: &mut impl Write, value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() + 1 > MAX_FRAME_BYTES {
        bail!("codemode IPC frame exceeds size limit");
    }
    writer.write_all(&bytes)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}
