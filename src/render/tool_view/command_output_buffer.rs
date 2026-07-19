use crate::render::terminal_text as t;
use std::borrow::Cow;
use std::collections::VecDeque;

const MAX_RETAINED_BYTES: usize = 80 * 1024;
const HEAD_BYTES: usize = MAX_RETAINED_BYTES / 2;

/// 保存命令输出的稳定开头与最新结尾，限制 TUI 状态占用。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct CommandOutputBuffer {
    head: Vec<u8>,
    tail: VecDeque<u8>,
    omitted_bytes: usize,
}

impl CommandOutputBuffer {
    /// 追加命令输出，并在超过容量时省略中间字节。
    ///
    /// 参数:
    /// - `chunk`: 新收到的原始输出
    /// - `omitted_bytes`: 上游批次省略的字节数
    ///
    /// 返回:
    /// - 无
    pub(crate) fn append(&mut self, chunk: &[u8], omitted_bytes: usize) {
        self.omitted_bytes = self.omitted_bytes.saturating_add(omitted_bytes);
        if chunk.is_empty() {
            return;
        }
        let head_remaining = HEAD_BYTES.saturating_sub(self.head.len());
        let head_len = head_remaining.min(chunk.len());
        self.head.extend_from_slice(&chunk[..head_len]);
        self.tail.extend(&chunk[head_len..]);
        let tail_limit = MAX_RETAINED_BYTES.saturating_sub(HEAD_BYTES);
        let excess = self.tail.len().saturating_sub(tail_limit);
        if excess > 0 {
            self.tail.drain(..excess);
            self.omitted_bytes = self.omitted_bytes.saturating_add(excess);
        }
    }

    /// 判断缓冲是否没有任何输出。
    ///
    /// 返回:
    /// - 没有保留字节时返回 true
    pub(crate) fn is_empty(&self) -> bool {
        self.head.is_empty() && self.tail.is_empty() && self.omitted_bytes == 0
    }

    /// 生成包含省略标记的可显示文本。
    ///
    /// 返回:
    /// - UTF-8 损失转换后的命令输出
    pub(crate) fn display_text(&self) -> Cow<'_, str> {
        if self.omitted_bytes == 0 {
            if self.tail.is_empty() {
                return String::from_utf8_lossy(&self.head);
            }
            let mut bytes = Vec::with_capacity(self.head.len().saturating_add(self.tail.len()));
            bytes.extend_from_slice(&self.head);
            bytes.extend(self.tail.iter().copied());
            return Cow::Owned(String::from_utf8_lossy(&bytes).into_owned());
        }
        Cow::Owned(format!(
            "{}\n... {} {} {} ...\n{}",
            String::from_utf8_lossy(&self.head),
            t("omitted", "已省略"),
            self.omitted_bytes,
            t("bytes", "字节"),
            String::from_utf8_lossy(&self.tail.iter().copied().collect::<Vec<_>>())
        ))
    }

    #[cfg(test)]
    /// 返回当前保留字节数量。
    ///
    /// 返回:
    /// - 缓冲内存使用量
    pub(super) fn retained_bytes(&self) -> usize {
        self.head.len().saturating_add(self.tail.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_head_and_tail_with_bounded_memory() {
        let mut buffer = CommandOutputBuffer::default();
        buffer.append(&vec![b'a'; MAX_RETAINED_BYTES], 0);
        buffer.append(b"latest", 0);

        let displayed = buffer.display_text();
        assert_eq!(buffer.retained_bytes(), MAX_RETAINED_BYTES);
        assert!(displayed.starts_with('a'));
        assert!(displayed.contains("6"));
        assert!(displayed.ends_with("latest"));
    }
}
