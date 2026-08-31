//! IPC 帧编解码。
//!
//! 线上格式是 **JSONL**（一行一个 JSON 对象，行尾 `\n`）。选它而不是长度前缀，是为了和
//! 项目既有的 journal JSONL 落盘格式保持一致，方便直接 `cat`/`tail` 排查问题。
//!
//! 读取走 [`tokio::io::AsyncBufReadExt::fill_buf`] + `consume`，而不是无界增长的
//! `read_until`：这样可以在流式读取的每一轮检查累计长度，单行超过 [`MAX_FRAME_BYTES`]
//! 时立刻返回错误，避免损坏/恶意对端把内存打满。

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

/// 单行帧的字节上限（8 MiB）。
///
/// 事件帧的 payload 里可能带工具输出，给到 8 MiB 足够宽松；超过这个值基本可以断定是
/// 对端写坏了或者故意灌数据。
pub(crate) const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

// ---------------------------------------------------------------------------
// 控制帧 kind 常量
// ---------------------------------------------------------------------------
// P1 只做编解码，不做业务语义。这些常量先定义出来，P3/P4 接入事件订阅与权限交互后
// 会成为真正的调用方。其中 P1 就已经用到的（hello / snapshot / ping / evt.runner）
// 不加 allow，剩下几个暂时没有引用，逐个标注以免 dead_code 噪音掩盖真实问题。

/// 观察者建连后的第一条帧：带上协议版本与自身角色。
pub(crate) const KIND_CTL_HELLO: &str = "ctl.hello";
/// 观察者请求订阅事件流（可携带起始 sequence 做断点补发）。
// P3 接入事件订阅后会有调用方。
#[allow(dead_code)]
pub(crate) const KIND_CTL_SUBSCRIBE: &str = "ctl.subscribe";
/// 持有者下发给观察者的全量状态快照。
// 目前只有测试在构造这种帧；接入会话恢复流程后会有调用方。
#[allow(dead_code)]
pub(crate) const KIND_CTL_SNAPSHOT: &str = "ctl.snapshot";
/// 交互请求（权限审批、提问等）。
// P4 接入权限交互后会有调用方。
#[allow(dead_code)]
pub(crate) const KIND_CTL_INTER_REQUEST: &str = "ctl.inter.request";
/// 交互请求的应答。
// P4 接入权限交互后会有调用方。
#[allow(dead_code)]
pub(crate) const KIND_CTL_INTER_REPLY: &str = "ctl.inter.reply";
/// 观察者请求持有者代跑一轮（观察者自己不持有 Agent）。
pub(crate) const KIND_CTL_SUBMIT: &str = "ctl.submit";
/// 持有者对 [`KIND_CTL_SUBMIT`] 的受理回执。
pub(crate) const KIND_CTL_SUBMIT_ACK: &str = "ctl.submit_ack";
/// 观察者请求持有者中断一轮。
pub(crate) const KIND_CTL_ABORT: &str = "ctl.abort";
/// 心跳请求。
pub(crate) const KIND_CTL_PING: &str = "ctl.ping";
/// 心跳应答。
// P3 接入心跳保活后会有调用方。
#[allow(dead_code)]
pub(crate) const KIND_CTL_PONG: &str = "ctl.pong";
/// 事件帧：payload 里放 runner 事件体。
// 目前只有测试在构造这种帧；接入跨进程事件流后会有调用方。
#[allow(dead_code)]
pub(crate) const KIND_EVT_RUNNER: &str = "evt.runner";
/// 事件帧：payload 里放**已组装**的 WebEvent。
///
/// 双向复用同一个 kind：
/// - 持有者 → 观察者：实时事件与按序号补发的历史；
/// - 观察者 → 持有者：观察者自己那一轮产生的事件，交由持有者统一分配序号并落盘。
///
/// 这样整个会话的事件文件始终只有一个写者，序号不会跨进程撞车。
pub(crate) const KIND_EVT_MIRROR: &str = "evt.mirror";

/// IPC 帧：JSONL 里的一行。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct Frame {
    /// 帧类型，决定 payload 的解析方式。取值见上面的 `KIND_*` 常量。
    pub(crate) kind: String,
    /// 单调递增序号，供观察者做断点补发。控制帧没有序号。
    #[serde(default)]
    pub(crate) sequence: Option<u64>,
    /// 载荷；控制帧与事件帧共用这个字段。
    #[serde(default)]
    pub(crate) payload: serde_json::Value,
}

impl Frame {
    /// 构造一个控制帧（无序号）。
    // P3/P4 接入订阅与交互流程后会有调用方。
    #[allow(dead_code)]
    pub(crate) fn control(kind: &str, payload: serde_json::Value) -> Self {
        Self {
            kind: kind.to_string(),
            sequence: None,
            payload,
        }
    }

    /// 编码为一行 JSON（**不含**结尾换行）。
    pub(crate) fn encode(&self) -> Result<Vec<u8>> {
        let mut line = serde_json::to_vec(self)?;
        if line.len() > MAX_FRAME_BYTES {
            return Err(anyhow!(
                "ipc 帧编码后 {} 字节，超过单行上限 {} 字节",
                line.len(),
                MAX_FRAME_BYTES
            ));
        }
        // serde_json 默认输出不含换行，这里只是防御：payload 里的字符串如果带 '\n'
        // 会被转义成 \\n，不会影响分帧。
        if line.last() == Some(&b'\n') {
            line.pop();
        }
        Ok(line)
    }

    /// 从一行 JSON 解码（自动剥掉结尾换行）。
    pub(crate) fn decode(line: &[u8]) -> Result<Self> {
        let bytes = match line.last() {
            Some(b'\n') => &line[..line.len() - 1],
            _ => line,
        };
        if bytes.len() > MAX_FRAME_BYTES {
            return Err(anyhow!(
                "ipc 帧 {} 字节，超过单行上限 {} 字节",
                bytes.len(),
                MAX_FRAME_BYTES
            ));
        }
        Ok(serde_json::from_slice(bytes)?)
    }
}

/// 从 `AsyncBufRead` 读一帧；流干净关闭（EOF，且没有读到半帧）返回 `Ok(None)`。
// P3/P4 接入 SessionActor 后会有调用方。
#[allow(dead_code)]
pub(crate) async fn read_frame<R>(reader: &mut R) -> Result<Option<Frame>>
where
    R: AsyncBufRead + Unpin,
{
    let mut line: Vec<u8> = Vec::new();

    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            // EOF。已经攒了半帧说明对端在帧中间断开，这是传输错误而不是正常关闭。
            if line.is_empty() {
                return Ok(None);
            }
            return Err(anyhow!(
                "ipc 流在帧中间关闭：已读到 {} 字节但未见换行",
                line.len()
            ));
        }

        match available.iter().position(|&b| b == b'\n') {
            Some(pos) => {
                line.extend_from_slice(&available[..pos]);
                let consumed = pos + 1;
                reader.consume(consumed);
                return Ok(Some(Frame::decode(&line)?));
            }
            None => {
                let len = available.len();
                line.extend_from_slice(available);
                reader.consume(len);
                if line.len() > MAX_FRAME_BYTES {
                    return Err(anyhow!(
                        "ipc 单行帧超过上限 {} 字节，判定为损坏或恶意对端，断开连接",
                        MAX_FRAME_BYTES
                    ));
                }
            }
        }
    }
}

/// 写一帧并 flush。
// P3/P4 接入 SessionActor 后会有调用方。
#[allow(dead_code)]
pub(crate) async fn write_frame<W>(writer: &mut W, frame: &Frame) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    let mut line = frame.encode()?;
    line.push(b'\n');
    writer.write_all(&line).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use serde_json::json;
    use tokio::io::BufReader;

    fn frame(kind: &str, seq: Option<u64>) -> Frame {
        Frame {
            kind: kind.to_string(),
            sequence: seq,
            payload: json!({"n": seq.unwrap_or(0)}),
        }
    }

    #[test]
    fn encode_decode_roundtrip() -> Result<()> {
        let f = frame(KIND_EVT_RUNNER, Some(7));
        let line = f.encode()?;
        // 编码结果不含结尾换行，保证 write_frame 是唯一加 '\n' 的地方。
        assert!(!line.ends_with(b"\n"));
        assert_eq!(Frame::decode(&line)?, f);

        // decode 要能容忍调用方传入带换行的原始行。
        let mut with_nl = line.clone();
        with_nl.push(b'\n');
        assert_eq!(Frame::decode(&with_nl)?, f);
        Ok(())
    }

    #[test]
    fn control_frame_carries_kind_and_payload_only() -> Result<()> {
        let f = Frame::control(KIND_CTL_PING, json!({"t": 42}));
        assert_eq!(f.kind, KIND_CTL_PING);
        assert_eq!(f.sequence, None);
        assert_eq!(f.payload, json!({"t": 42}));

        // 缺省字段要能从最小 JSON 解出来（kind-only）。
        let decoded = Frame::decode(br#"{"kind":"ctl.hello"}"#)?;
        assert_eq!(decoded.kind, KIND_CTL_HELLO);
        assert_eq!(decoded.sequence, None);
        assert_eq!(decoded.payload, serde_json::Value::Null);
        Ok(())
    }

    /// 一次写入多帧，必须能逐帧读出，且顺序一致。
    #[tokio::test]
    async fn multiple_frames_in_one_write_are_read_back_in_order() -> Result<()> {
        let mut buf: Vec<u8> = Vec::new();
        for i in 0..5u64 {
            let mut line = frame(KIND_EVT_RUNNER, Some(i)).encode()?;
            line.push(b'\n');
            buf.extend_from_slice(&line);
        }

        let mut reader = BufReader::new(&buf[..]);
        for i in 0..5u64 {
            assert_eq!(read_frame(&mut reader).await?, Some(frame(KIND_EVT_RUNNER, Some(i))));
        }
        // 读完应干净 EOF。
        assert_eq!(read_frame(&mut reader).await?, None);
        Ok(())
    }

    /// 一帧分多次写入（模拟 UDS/管道分片），要能正确拼回。
    #[tokio::test]
    async fn frame_split_across_writes_is_reassembled() -> Result<()> {
        let big_payload = json!({"blob": "x".repeat(64 * 1024)});
        let f = Frame {
            kind: KIND_CTL_SNAPSHOT.to_string(),
            sequence: Some(1),
            payload: big_payload,
        };
        let line = f.encode()?;

        // 用极小的管道（每段 1 KiB）喂进去，强制 fill_buf 分多轮返回。
        let (writer, reader) = tokio::io::duplex(1024);
        let mut reader = BufReader::new(reader);
        let writer_task = tokio::spawn(async move {
            let mut writer = writer;
            // 按奇数长度切片，跨过换行边界。
            for chunk in line.chunks(999) {
                tokio::io::AsyncWriteExt::write_all(&mut writer, chunk).await?;
            }
            tokio::io::AsyncWriteExt::write_all(&mut writer, b"\n").await?;
            tokio::io::AsyncWriteExt::flush(&mut writer).await?;
            Ok::<(), anyhow::Error>(())
        });

        assert_eq!(read_frame(&mut reader).await?, Some(f));
        writer_task.await??;
        Ok(())
    }

    /// 超长帧（没有换行）必须触发上限错误，而不是无限缓冲或 panic。
    #[tokio::test]
    async fn oversized_line_errors_instead_of_buffering_forever() -> Result<()> {
        let (mut writer, reader) = tokio::io::duplex(64 * 1024);
        let mut reader = BufReader::new(reader);

        // 只灌 'a' 不换行，累计超过 8 MiB。
        let feeder = tokio::spawn(async move {
            let chunk = vec![b'a'; 64 * 1024];
            for _ in 0..(MAX_FRAME_BYTES / (64 * 1024) + 4) {
                if tokio::io::AsyncWriteExt::write_all(&mut writer, &chunk).await.is_err() {
                    return;
                }
            }
            let _ = tokio::io::AsyncWriteExt::flush(&mut writer).await;
        });

        let err = read_frame(&mut reader).await.expect_err("超长行必须报错");
        assert!(
            err.to_string().contains("超过上限"),
            "错误信息应当点明是超上限，实际是: {err}"
        );

        // 连接被我们主动放弃，feeder 会在写入时收到 EPIPE 后退出。
        drop(reader);
        let _ = feeder.await;
        Ok(())
    }

    /// encode 侧同样要挡住超限帧，避免写出一个对端读不了的巨帧。
    #[test]
    fn encode_rejects_oversized_frame() -> Result<()> {
        let f = Frame {
            kind: KIND_EVT_RUNNER.to_string(),
            sequence: None,
            payload: json!({"blob": "x".repeat(MAX_FRAME_BYTES + 1024)}),
        };
        assert!(f.encode().is_err());
        Ok(())
    }

    /// 对端在帧中间关闭：应当报错，而不是假装收到一帧。
    #[tokio::test]
    async fn truncated_frame_at_eof_is_an_error() -> Result<()> {
        let buf = b"{\"kind\":\"evt.runner\"".to_vec();
        let mut reader = BufReader::new(&buf[..]);
        let err = read_frame(&mut reader).await.expect_err("半帧 + EOF 必须报错");
        assert!(err.to_string().contains("帧中间关闭"), "实际错误: {err}");
        Ok(())
    }

    /// 写入后立即 flush：用 duplex 的最小缓冲验证对端能马上读到完整一帧。
    #[tokio::test]
    async fn write_frame_flushes() -> Result<()> {
        let (client, server) = tokio::io::duplex(64);
        let mut client = BufReader::new(client);
        let mut server = server;

        write_frame(&mut server, &Frame::control(KIND_CTL_HELLO, json!({}))).await?;
        let got = read_frame(&mut client).await?;
        assert_eq!(got, Some(Frame::control(KIND_CTL_HELLO, json!({}))));
        Ok(())
    }
}
