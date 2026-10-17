use std::collections::VecDeque;

pub(super) const TERMINAL_REPLAY_BYTES: usize = 1024 * 1024;

/// 带字节偏移的终端输出分片。
///
/// 广播通道在接收端跟不上时会丢掉中间分片。偏移用来从回放缓冲把缺口补回去，
/// 避免 zsh 这类输出很密的 Shell 在屏幕上随机少字。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalChunk {
    /// 该分片第一个字节在会话输出中的偏移
    pub start: u64,
    pub bytes: Vec<u8>,
}

/// 终端输出回放缓冲。
///
/// 只保留最近一段字节，同时记录会话累计输出量，供滞后的连接补齐缺口。
#[derive(Debug)]
pub(super) struct ReplayBuffer {
    bytes: VecDeque<u8>,
    total: u64,
}

impl ReplayBuffer {
    /// 创建空的回放缓冲。
    ///
    /// 返回:
    /// - 空缓冲
    pub(super) fn new() -> Self {
        Self {
            bytes: VecDeque::new(),
            total: 0,
        }
    }

    /// 追加一段输出并返回可广播的分片。
    ///
    /// 参数:
    /// - `chunk`: 新读到的终端字节
    ///
    /// 返回:
    /// - 带起始偏移的分片
    pub(super) fn push(&mut self, chunk: &[u8]) -> TerminalChunk {
        let start = self.total;
        self.bytes.extend(chunk.iter().copied());
        self.total = self.total.saturating_add(chunk.len() as u64);
        while self.bytes.len() > TERMINAL_REPLAY_BYTES {
            self.bytes.pop_front();
        }
        TerminalChunk {
            start,
            bytes: chunk.to_vec(),
        }
    }

    /// 返回当前仍保留的全部回放字节和累计偏移。
    ///
    /// 返回:
    /// - 回放字节，以及这些字节结束后的累计偏移
    #[allow(dead_code)]
    pub(super) fn snapshot(&self) -> (Vec<u8>, u64) {
        (self.bytes.iter().copied().collect(), self.total)
    }

    /// 返回指定偏移之后仍保留的字节，以及最新累计偏移。
    ///
    /// 早于保留窗口的字节无法找回，调用方应把游标推进到返回的累计偏移。
    ///
    /// 参数:
    /// - `cursor`: 连接已经写给前端的字节偏移
    ///
    /// 返回:
    /// - 需要补写的字节，以及补写后应使用的累计偏移
    pub(super) fn since(&self, cursor: u64) -> (Vec<u8>, u64) {
        let retained_start = self.total.saturating_sub(self.bytes.len() as u64);
        let from = cursor.max(retained_start).min(self.total);
        let skip = usize::try_from(from.saturating_sub(retained_start)).unwrap_or(usize::MAX);
        (
            self.bytes.iter().skip(skip).copied().collect(),
            self.total,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【回放】滞后连接能从偏移补回被广播丢掉的字节。
    #[test]
    fn since_returns_only_bytes_after_the_cursor() {
        let mut replay = ReplayBuffer::new();
        replay.push(b"hello ");
        replay.push(b"zsh");

        let (gap, total) = replay.since(6);

        assert_eq!(gap, b"zsh");
        assert_eq!(total, 9);
        assert_eq!(replay.snapshot().0, b"hello zsh");
    }

    /// 【终端】【回放】超出保留窗口的旧字节不再重复发送。
    #[test]
    fn since_skips_bytes_that_fell_out_of_the_window() {
        let mut replay = ReplayBuffer {
            bytes: VecDeque::new(),
            total: 0,
        };
        replay.push(&vec![b'a'; TERMINAL_REPLAY_BYTES + 4]);
        let cursor_inside = replay.total - 3;

        let (gap, total) = replay.since(0);
        let (tail, _) = replay.since(cursor_inside);

        assert_eq!(gap.len(), TERMINAL_REPLAY_BYTES);
        assert_eq!(total, replay.total);
        assert_eq!(tail, b"aaa");
    }
}
