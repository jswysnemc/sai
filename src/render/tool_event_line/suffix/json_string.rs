//! 【工具参数】【JSON 字符串】只解码已完整收到的字符，保留跨分片转义边界。

/// 【工具参数】【前缀解码】按 JSON 规则解码闭合字符串或尚未闭合的有效前缀。
/// 参数: `value` 为以双引号开头的原始 JSON 字符串；返回: 已解码前缀，非法转义返回空值。
pub(super) fn decode_prefix(value: &str) -> Option<String> {
    if !value.starts_with('"') {
        return None;
    }
    let bytes = value.as_bytes();
    let mut index = 1;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => return serde_json::from_str(&value[..=index]).ok(),
            b'\\' => {
                // 1. 【工具参数】【转义边界】未收全的转义留待下一分片，不丢弃路径分隔符
                let Some(&escape) = bytes.get(index + 1) else {
                    break;
                };
                match escape {
                    b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => index += 2,
                    b'u' => {
                        let Some(unit) = unicode_unit(bytes, index)? else {
                            break;
                        };
                        if (0xd800..=0xdbff).contains(&unit) {
                            let Some(low) = unicode_unit(bytes, index + 6)? else {
                                break;
                            };
                            if !(0xdc00..=0xdfff).contains(&low) {
                                return None;
                            }
                            index += 12;
                        } else if (0xdc00..=0xdfff).contains(&unit) {
                            return None;
                        } else {
                            index += 6;
                        }
                    }
                    _ => return None,
                }
            }
            0..=0x1f => return None,
            _ => index += value[index..].chars().next()?.len_utf8(),
        }
    }
    // 2. 【工具参数】【标准解码】只补闭合引号，实际反转义由 serde_json 处理，避免二次解码
    serde_json::from_str(&format!("{}\"", &value[..index])).ok()
}

/// 【工具参数】【Unicode 边界】读取完整的四位转义，不把未闭合代理对转换成替换字符。
/// 参数: `bytes` 为 JSON 字符串字节，`start` 为反斜杠位置；返回: 完整码元为 `Some(Some(..))`，待续为 `Some(None)`，非法输入为 `None`。
fn unicode_unit(bytes: &[u8], start: usize) -> Option<Option<u16>> {
    let Some(tail) = bytes.get(start..) else {
        return Some(None);
    };
    if !b"\\u".starts_with(&tail[..tail.len().min(2)]) {
        return None;
    }
    let digits = &tail[tail.len().min(2)..tail.len().min(6)];
    if !digits.iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    if tail.len() < 6 {
        return Some(None);
    }
    let text = std::str::from_utf8(digits).ok()?;
    Some(Some(u16::from_str_radix(text, 16).ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【工具参数】【分片回归】所有字符边界均返回原文前缀，反斜杠、中文及代理对不损坏。
    /// 参数: 无；返回: 无，任意分片解码破坏路径或引号时断言失败。
    #[test]
    fn every_fragment_boundary_preserves_original_prefix() {
        for encoded in [
            r#""D:\\work\\11\u6708 \\attachments\\extracted\"""#,
            r#""\\\\server\\share\\test\uD83D\uDCC1.ps1""#,
            r#""路径\\new\\test\n\r\t\b\f\/end""#,
        ] {
            let source: String = serde_json::from_str(encoded).unwrap();
            for end in (1..=encoded.len()).filter(|end| encoded.is_char_boundary(*end)) {
                let prefix = decode_prefix(&encoded[..end]).unwrap();
                assert!(
                    source.starts_with(&prefix),
                    "end={end} prefix={prefix:?} source={source:?}"
                );
            }
            assert_eq!(decode_prefix(encoded).unwrap(), source);
        }
    }

    /// 【工具参数】【非法转义】非法路径转义和不配对代理字符不能静默删除反斜杠。
    /// 参数: 无；返回: 无，非法序列被损坏解码时断言失败。
    #[test]
    fn invalid_escapes_are_rejected_without_lossy_decoding() {
        for source in [
            r#""D:\Code""#,
            r#""\uDC00""#,
            r#""\uD800\u0041""#,
            r#""\uXYZ0""#,
        ] {
            assert_eq!(decode_prefix(source), None, "{source:?}");
        }
    }
}
