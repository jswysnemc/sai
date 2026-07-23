use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

const COMMAND_OUTPUT_PREFIX: &str = "__sai_command_output__";
const PROGRESS_BATCH_BYTES: usize = 32 * 1024;
const PROGRESS_BATCH_HEAD_BYTES: usize = PROGRESS_BATCH_BYTES / 2;

/// 命令输出流类型。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CommandOutputStream {
    Stdout,
    Stderr,
}

/// 命令执行期间产生的原始输出片段。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct CommandOutputChunk {
    pub(crate) stream: CommandOutputStream,
    pub(crate) bytes: Vec<u8>,
    pub(crate) omitted_bytes: usize,
}

/// 命令输出片段的内部传输载荷。
#[derive(Deserialize, Serialize)]
struct EncodedCommandOutputChunk {
    stream: CommandOutputStream,
    data: String,
    #[serde(default)]
    omitted_bytes: usize,
}

/// 汇总一个刷新周期内的命令输出，固定保留开头与结尾。
#[derive(Default)]
pub(super) struct CommandOutputBatch {
    head: Vec<u8>,
    tail: VecDeque<u8>,
    omitted_bytes: usize,
}

impl CommandOutputBatch {
    /// 追加当前刷新周期收到的命令输出。
    ///
    /// 参数:
    /// - `chunk`: 原始输出片段
    ///
    /// 返回:
    /// - 无
    pub(super) fn append(&mut self, chunk: &[u8]) {
        if chunk.is_empty() {
            return;
        }
        let head_remaining = PROGRESS_BATCH_HEAD_BYTES.saturating_sub(self.head.len());
        let head_len = head_remaining.min(chunk.len());
        self.head.extend_from_slice(&chunk[..head_len]);
        self.tail.extend(&chunk[head_len..]);
        let tail_limit = PROGRESS_BATCH_BYTES.saturating_sub(PROGRESS_BATCH_HEAD_BYTES);
        let excess = self.tail.len().saturating_sub(tail_limit);
        if excess > 0 {
            self.tail.drain(..excess);
            self.omitted_bytes = self.omitted_bytes.saturating_add(excess);
        }
    }

    /// 判断当前批次是否没有输出。
    ///
    /// 返回:
    /// - 没有保留或省略字节时返回 true
    pub(super) fn is_empty(&self) -> bool {
        self.head.is_empty() && self.tail.is_empty() && self.omitted_bytes == 0
    }

    /// 生成内部进度消息并清空当前批次。
    ///
    /// 参数:
    /// - `stream`: 输出流类型
    ///
    /// 返回:
    /// - 空批次返回空值，否则返回编码后的进度消息
    pub(super) fn take_message(&mut self, stream: CommandOutputStream) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let mut bytes = Vec::with_capacity(PROGRESS_BATCH_BYTES);
        bytes.append(&mut self.head);
        bytes.extend(self.tail.drain(..));
        let omitted_bytes = std::mem::take(&mut self.omitted_bytes);
        Some(encode_command_output_with_omission(
            stream,
            &bytes,
            omitted_bytes,
        ))
    }
}

/// 将命令输出片段编码为内部工具进度消息。
///
/// 参数:
/// - `stream`: 输出流类型
/// - `bytes`: 原始输出字节
///
/// 返回:
/// - 可通过 ToolProgress 传递的内部消息
pub(crate) fn encode_command_output(stream: CommandOutputStream, bytes: &[u8]) -> String {
    encode_command_output_with_omission(stream, bytes, 0)
}

/// 将带省略计数的命令输出片段编码为内部进度消息。
///
/// 参数:
/// - `stream`: 输出流类型
/// - `bytes`: 保留的原始输出
/// - `omitted_bytes`: 当前批次省略的字节数
///
/// 返回:
/// - 可通过 ToolProgress 传递的内部消息
fn encode_command_output_with_omission(
    stream: CommandOutputStream,
    bytes: &[u8],
    omitted_bytes: usize,
) -> String {
    let payload = EncodedCommandOutputChunk {
        stream,
        data: STANDARD.encode(bytes),
        omitted_bytes,
    };
    format!(
        "{COMMAND_OUTPUT_PREFIX}{}",
        serde_json::to_string(&payload).unwrap_or_default()
    )
}

/// 解析内部命令输出进度消息。
///
/// 参数:
/// - `message`: 工具进度消息
///
/// 返回:
/// - 命令输出片段；普通工具进度返回空
pub(crate) fn decode_command_output(message: &str) -> Option<CommandOutputChunk> {
    let payload = message.strip_prefix(COMMAND_OUTPUT_PREFIX)?;
    let payload = serde_json::from_str::<EncodedCommandOutputChunk>(payload).ok()?;
    Some(CommandOutputChunk {
        stream: payload.stream,
        bytes: STANDARD.decode(payload.data).ok()?,
        omitted_bytes: payload.omitted_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_output_chunk_round_trips_binary_bytes() {
        let message = encode_command_output(CommandOutputStream::Stdout, &[0, 1, 0xff]);
        let chunk = decode_command_output(&message).unwrap();

        assert_eq!(chunk.stream, CommandOutputStream::Stdout);
        assert_eq!(chunk.bytes, vec![0, 1, 0xff]);
        assert_eq!(chunk.omitted_bytes, 0);
        assert!(!message.contains("255"));
    }

    #[test]
    fn rejects_invalid_command_output_payload() {
        let message = format!(
            "{COMMAND_OUTPUT_PREFIX}{}",
            r#"{"stream":"stdout","data":"%%%"}"#
        );

        assert!(decode_command_output(&message).is_none());
    }

    #[test]
    fn command_output_batch_bounds_high_volume_progress() {
        let mut batch = CommandOutputBatch::default();
        let mut output = b"start\n".to_vec();
        output.extend(vec![b'x'; PROGRESS_BATCH_BYTES * 4]);
        output.extend_from_slice(b"\nfinish");
        batch.append(&output);

        let message = batch.take_message(CommandOutputStream::Stdout).unwrap();
        let chunk = decode_command_output(&message).unwrap();
        let text = String::from_utf8_lossy(&chunk.bytes);

        assert!(chunk.bytes.len() <= PROGRESS_BATCH_BYTES + 64);
        assert!(text.starts_with("start"));
        assert!(text.ends_with("finish"));
        assert!(chunk.omitted_bytes > 0);
        assert!(batch.is_empty());
    }
}
