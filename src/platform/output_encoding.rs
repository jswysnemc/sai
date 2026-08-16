/// 将命令输出字节解码为可展示文本。
///
/// 参数:
/// - `bytes`: 子进程输出的原始字节
///
/// 返回:
/// - 解码后的 UTF-8 文本
///
/// Windows PowerShell 已由调用入口设置为 UTF-8；兼容用户配置的 cmd、
/// 旧脚本及系统代码页时，UTF-8 失败后按 Windows 常见的 GBK 解码，
/// 最后才使用替换字符兜底。
pub(crate) fn decode_output(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_owned();
    }
    #[cfg(windows)]
    {
        let (text, _, _) = encoding_rs::GBK.decode(bytes);
        return text.into_owned();
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// 跨读取块保持编码边界的命令输出解码器。
#[derive(Default)]
pub(crate) struct OutputDecoder {
    pending: Vec<u8>,
}

impl OutputDecoder {
    /// 解码一个输出块，保留尚未完整到达的 UTF-8 尾部。
    ///
    /// 参数:
    /// - `bytes`: 本次读取到的原始输出
    ///
    /// 返回:
    /// - 当前可展示文本
    pub(crate) fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let result = match std::str::from_utf8(&self.pending) {
            Ok(text) => {
                let text = text.to_owned();
                self.pending.clear();
                text
            }
            Err(error) if error.error_len().is_none() => {
                let valid = error.valid_up_to();
                let text = decode_output(&self.pending[..valid]);
                self.pending.drain(..valid);
                text
            }
            Err(_) => {
                let text = decode_output(&self.pending);
                self.pending.clear();
                text
            }
        };
        result
    }

    /// 刷新流末尾仍未组成完整字符的字节。
    ///
    /// 返回:
    /// - 末尾文本
    pub(crate) fn finish(&mut self) -> String {
        if self.pending.is_empty() {
            String::new()
        } else {
            let text = decode_output(&self.pending);
            self.pending.clear();
            text
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_output, OutputDecoder};

    #[test]
    fn valid_utf8_is_preserved() {
        assert_eq!(decode_output("中文".as_bytes()), "中文");
    }

    #[test]
    fn decoder_preserves_characters_split_across_chunks() {
        let bytes = "中文".as_bytes();
        let mut decoder = OutputDecoder::default();
        let mut text = decoder.push(&bytes[..1]);
        text.push_str(&decoder.push(&bytes[1..3]));
        text.push_str(&decoder.push(&bytes[3..]));
        text.push_str(&decoder.finish());
        assert_eq!(text, "中文");
    }

    #[cfg(windows)]
    #[test]
    fn gbk_output_is_decoded_on_windows() {
        let (encoded, _, _) = encoding_rs::GBK.encode("中文");
        assert_eq!(decode_output(&encoded), "中文");
    }
}
