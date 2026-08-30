use crate::render::terminal_paint::paint_lock;
use std::io::{self, Write};

/// 开始同步更新：终端在收到结束序列前不把改动提交到屏幕。
const BEGIN_SYNC: &[u8] = b"\x1b[?2026h";
/// 结束同步更新，整帧一次性提交。
const END_SYNC: &[u8] = b"\x1b[?2026l";

/// 一帧终端绘制的输出缓冲。
///
/// TUI 每 32ms 重画一次，一帧包含腾行滚动、历史行修补与 composer 重绘三段
/// 输出。早先每段各自 flush，而 Windows Terminal 的渲染线程与写入完全解耦，
/// 会抓到两次 flush 之间的中间态：清行后尚未打印的空行表现为状态行闪动，
/// 滚动后 composer 尚未重绘表现为底栏上下跳动。
///
/// 缓冲整帧后一次写出，并用同步更新序列包裹；支持该序列的终端会把整帧
/// 当作一次原子提交，不支持的终端会忽略这两条私有模式序列。
pub(crate) struct TerminalFrame {
    buffer: Vec<u8>,
}

impl TerminalFrame {
    /// 创建空的帧缓冲。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 新的帧缓冲
    pub(crate) fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(8192),
        }
    }

    /// 把当前帧一次性写入终端并清空缓冲。
    ///
    /// 步骤:
    /// 1. 空帧直接返回，不产生任何终端输出
    /// 2. 取绘制帧锁，避免与动画线程的直写交错
    /// 3. 同步更新序列包裹整帧内容后写出并 flush
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 写入结果
    pub(crate) fn commit(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let payload = framed(&self.buffer);
        let _paint = paint_lock();
        let mut stdout = io::stdout().lock();
        stdout.write_all(&payload)?;
        stdout.flush()?;
        self.buffer.clear();
        Ok(())
    }
}

/// 用同步更新序列包裹一帧内容。
///
/// 参数:
/// - `buffer`: 本帧累积的绘制字节
///
/// 返回:
/// - 可直接写入终端的完整帧
fn framed(buffer: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(buffer.len() + BEGIN_SYNC.len() + END_SYNC.len());
    payload.extend_from_slice(BEGIN_SYNC);
    payload.extend_from_slice(buffer);
    payload.extend_from_slice(END_SYNC);
    payload
}

impl Default for TerminalFrame {
    fn default() -> Self {
        Self::new()
    }
}

impl Write for TerminalFrame {
    /// 把绘制字节追加到帧缓冲。
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    /// 帧内 flush 是空操作：真正的写出只发生在 `commit`。
    ///
    /// 绘制函数内部保留了各自的 flush 调用，落到帧缓冲上不再产生系统调用，
    /// 中间态因此不会出现在屏幕上。
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【帧缓冲】验证写入只进缓冲，flush 不清空内容。
    #[test]
    fn writes_accumulate_and_flush_keeps_them() {
        let mut frame = TerminalFrame::new();
        write!(frame, "first").unwrap();
        frame.flush().unwrap();
        write!(frame, "second").unwrap();

        assert_eq!(frame.buffer, b"firstsecond");
    }

    /// 【终端】【帧缓冲】验证空帧提交不写终端。
    #[test]
    fn committing_an_empty_frame_is_a_no_op() {
        let mut frame = TerminalFrame::new();

        frame.commit().unwrap();

        assert!(frame.buffer.is_empty());
    }

    /// 【终端】【帧缓冲】验证整帧被同步更新序列完整包裹。
    ///
    /// 包裹之后终端把清行与打印当作一次提交，中间态不会上屏——
    /// 状态行闪动与底栏跳动都源自这一中间态。
    #[test]
    fn frame_payload_is_wrapped_in_synchronized_update() {
        let payload = framed(b"\x1b[2Kline");

        assert!(payload.starts_with(BEGIN_SYNC));
        assert!(payload.ends_with(END_SYNC));
        let inner = &payload[BEGIN_SYNC.len()..payload.len() - END_SYNC.len()];
        assert_eq!(inner, b"\x1b[2Kline");
    }
}
