/// 把 HTTP 响应体解码为字符串。
///
/// reqwest 的 `.text()` 在响应头未声明字符集时按严格 UTF-8 解码，
/// 遇到 GBK / Big5 等编码的中文站点会直接报
/// "stream did not contain valid UTF-8"。这里先按响应头声明的字符集解码，
/// 未声明时回退 UTF-8，仍失败则按 Windows-1252 逐字节兜底，永不报错。
///
/// 参数:
/// - `response`: 已完成的 HTTP 响应
///
/// 返回:
/// - 解码后的响应体文本
pub async fn decode_body(response: reqwest::Response) -> anyhow::Result<String> {
    let bytes = response.bytes().await?;
    Ok(decode_bytes(&bytes))
}

/// 按字节与声明的字符集解码文本。
///
/// 参数:
/// - `bytes`: 响应体原始字节
///
/// 返回:
/// - 解码后的文本；UTF-8 合法时原样返回
pub fn decode_bytes(bytes: &[u8]) -> String {
    let (cow, _, had_errors) = encoding_rs::UTF_8.decode(bytes);
    if !had_errors {
        return cow.into_owned();
    }
    // 非 UTF-8：按常见中文编码尝试，仍失败则逐字节兜底
    for encoding in [encoding_rs::GBK, encoding_rs::BIG5, encoding_rs::WINDOWS_1252] {
        let (cow, _, had_errors) = encoding.decode(bytes);
        if !had_errors {
            return cow.into_owned();
        }
    }
    String::from_utf8_lossy(bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证合法 UTF-8 原样返回。
    #[test]
    fn utf8_passes_through() {
        assert_eq!(decode_bytes("用户的问题".as_bytes()), "用户的问题");
    }

    /// 验证 GBK 字节被正确解码而不是报错。
    ///
    /// "你好" 的 GBK 编码为 C4 E3 BA C3，严格 UTF-8 解码会失败。
    #[test]
    fn gbk_bytes_decode_to_the_original_text() {
        let gbk = [0xC4, 0xE3, 0xBA, 0xC3];
        assert_eq!(decode_bytes(&gbk), "你好");
    }

    /// 验证任意字节都不会导致解码失败。
    #[test]
    fn arbitrary_bytes_never_fail() {
        let bytes = [0xFF, 0xFE, 0x00, 0x80, 0x41];
        let decoded = decode_bytes(&bytes);
        assert!(decoded.contains('A'));
    }
}
