use std::collections::VecDeque;
use std::io::Read;
use std::sync::{Arc, Mutex};

const OUTPUT_CAPACITY: usize = 128 * 1024;

/// 【交互命令】【输出缓冲】只保存尚未消费的尾部输出，累计记录省略字节数。
#[derive(Default)]
pub(super) struct OutputBuffer {
    bytes: VecDeque<u8>,
    omitted: usize,
    pub(super) eof: bool,
}

impl OutputBuffer {
    /// 【交互命令】【快照读取】复制未消费缓冲供管理面板展示；无参数，返回文本和截断标记。
    pub(super) fn peek(&self) -> (String, bool) {
        let bytes = self.bytes.iter().copied().collect::<Vec<_>>();
        (
            String::from_utf8_lossy(&bytes).into_owned(),
            self.omitted > 0,
        )
    }

    /// 【交互命令】【输出追加】容量不足时移除最早字节；参数为原始输出，返回无。
    fn append(&mut self, bytes: &[u8]) {
        for byte in bytes {
            if self.bytes.len() == OUTPUT_CAPACITY {
                self.bytes.pop_front();
                self.omitted += 1;
            }
            self.bytes.push_back(*byte);
        }
        // 1. 【交互命令】【字符边界】尾部缓冲不能从 UTF-8 延续字节开始
        if self.omitted > 0 {
            while self.bytes.front().is_some_and(|byte| byte & 0xc0 == 0x80) {
                self.bytes.pop_front();
                self.omitted += 1;
            }
        }
    }

    /// 【交互命令】【输出消费】保留尚未完整到达的 UTF-8 尾部；无参数，返回文本和省略数量。
    pub(super) fn take(&mut self) -> (String, usize) {
        let bytes = self.bytes.make_contiguous();
        let count = match std::str::from_utf8(bytes) {
            Err(error) if !self.eof && error.error_len().is_none() => error.valid_up_to(),
            _ => bytes.len(),
        };
        let text = String::from_utf8_lossy(&bytes[..count]).into_owned();
        self.bytes.drain(..count);
        (text, std::mem::take(&mut self.omitted))
    }
}

/// 【交互命令】【输出读取】专用线程持续读取 PTY；参数为读取端和共享缓冲，返回无。
pub(super) fn read_output(mut reader: Box<dyn Read + Send>, buffer: Arc<Mutex<OutputBuffer>>) {
    let mut bytes = [0; 8192];
    loop {
        match reader.read(&mut bytes) {
            Ok(0) => break,
            Ok(count) => buffer
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .append(&bytes[..count]),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    buffer.lock().unwrap_or_else(|error| error.into_inner()).eof = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【交互命令】【输出回归】分片中文不产生替换字符，大输出有界且明确返回截断数量。
    #[test]
    fn preserves_split_utf8_and_bounds_unconsumed_output() {
        let mut output = OutputBuffer::default();
        output.append(&"中文".as_bytes()[..2]);
        assert_eq!(output.take().0, "");
        output.append(&"中文".as_bytes()[2..]);
        assert_eq!(output.take().0, "中文");
        output.append(&vec![b'x'; OUTPUT_CAPACITY * 3]);
        let (text, omitted) = output.take();
        assert_eq!(text.len(), OUTPUT_CAPACITY);
        assert_eq!(omitted, OUTPUT_CAPACITY * 2);
    }
}
