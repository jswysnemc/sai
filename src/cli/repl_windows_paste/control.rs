use super::WindowsPasteKey;
use std::time::{Duration, Instant};

const CRLF_PAIR_WINDOW: Duration = Duration::from_millis(120);

/// 【终端】【粘贴换行】保留已消费 CR 对应的可选 LF，兼容终端合并或完整发送 CRLF
#[derive(Clone, Debug, Default)]
pub(super) struct PasteControl {
    pending_lf_until: Option<Instant>,
}

impl PasteControl {
    /// 【终端】【粘贴换行】登记本次原始换行，只有 CRLF 中的 CR 才等待配对 LF
    /// 参数: key 为当前换行键，crlf 为原文是否使用 CRLF，now 为事件时间；返回无
    pub(super) fn record_newline(&mut self, key: WindowsPasteKey, crlf: bool, now: Instant) {
        self.pending_lf_until =
            (key == WindowsPasteKey::Enter && crlf).then_some(now + CRLF_PAIR_WINDOW);
    }

    /// 【终端】【粘贴换行】消费 CRLF 的第二个事件，其他按键结束配对窗口
    /// 参数: key 为当前键，now 为接收时间；返回当前 LF 已与 CR 合并时为真
    pub(super) fn consume_paired_lf(&mut self, key: WindowsPasteKey, now: Instant) -> bool {
        self.consume_pair(key, now, false)
    }

    /// 【终端】【粘贴换行】原文尚未回放完成时保留配对，结束后按窗口保护正常快捷键
    /// 参数: key 为当前键，now 为时间，replaying 为原文是否尚未消费；返回配对成功时为真
    fn consume_pair(&mut self, key: WindowsPasteKey, now: Instant, replaying: bool) -> bool {
        let pending = self.pending_lf_until.take();
        key == WindowsPasteKey::LineFeed && pending.is_some_and(|until| replaying || now <= until)
    }

    /// 【终端】【粘贴换行】检查是否仍需读取已确认 CRLF 的可选 LF
    /// 参数: now 为读取时间；返回配对窗口仍有效时为真
    pub(super) fn has_pending_lf(&self, now: Instant) -> bool {
        self.pending_lf_until.is_some_and(|until| now <= until)
    }

    /// 【终端】【粘贴回放】匹配正文中的一个字符或完整原始换行
    /// 参数: text 为未消费原文，key 为当前键，now 为事件时间
    /// 返回: 消费的 UTF-8 字节数，配对 LF 为零，不匹配为空
    pub(super) fn matching_bytes(
        &mut self,
        text: &str,
        key: WindowsPasteKey,
        now: Instant,
    ) -> Option<usize> {
        // 1. 【终端】【粘贴换行】CRLF 的 LF 不再推进正文，也不能成为第二次提交
        if self.consume_pair(key, now, !text.is_empty()) {
            return Some(0);
        }
        match key {
            WindowsPasteKey::Enter | WindowsPasteKey::LineFeed => {
                let bytes = newline_bytes(text)?;
                self.record_newline(key, bytes == 2, now);
                Some(bytes)
            }
            WindowsPasteKey::Char(ch) => text.starts_with(ch).then_some(ch.len_utf8()),
            WindowsPasteKey::Tab => text.starts_with('\t').then_some(1),
        }
    }
}

/// 【终端】【原始换行】识别正文开头的 CRLF、CR 或 LF，避免丢失终端事件配对信息
/// 参数: text 为未消费原文；返回换行占用的字节数，非换行为空
pub(super) fn newline_bytes(text: &str) -> Option<usize> {
    if text.starts_with("\r\n") {
        Some(2)
    } else if text.starts_with(['\r', '\n']) {
        Some(1)
    } else {
        None
    }
}
