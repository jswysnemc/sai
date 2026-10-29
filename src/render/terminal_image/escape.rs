/// 返回从指定位置开始的完整终端转义序列结束位置。
///
/// 支持 CSI、OSC、DCS 与 APC，保证图片协议载荷不会被文本换行器拆分。
///
/// 参数:
/// - `text`: 原始终端文本
/// - `start`: ESC 所在字节位置
///
/// 返回:
/// - 转义序列后第一个字节位置；ESC 后不是 ASCII 类型时仅跳过 ESC，保留完整字符
pub(crate) fn escape_sequence_end(text: &str, start: usize) -> usize {
    scan_escape_sequence(text, start).end
}

/// 控制序列的字节边界及闭合状态，未闭合内容不能写入终端。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EscapeSequence {
    pub(crate) end: usize,
    pub(crate) complete: bool,
}

/// 【终端】【控制序列扫描】区分已闭合控制序列与分片尾部，非法字节留给可见文本处理。
/// 参数：text 为原始文本，start 为 ESC 字节位置；返回：安全结束边界和闭合状态。
pub(crate) fn scan_escape_sequence(text: &str, start: usize) -> EscapeSequence {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&0x1b) {
        return EscapeSequence {
            end: start,
            complete: false,
        };
    }
    let Some(kind) = bytes.get(start + 1).copied() else {
        return EscapeSequence {
            end: start + 1,
            complete: false,
        };
    };
    // 1. 【终端】【UTF-8 边界】多字节字符不能作为转义类型，固定跳过两字节会切断其编码
    if !kind.is_ascii() {
        return EscapeSequence {
            end: start + 1,
            complete: false,
        };
    }
    if kind == b'[' {
        return csi_end(bytes, start + 2);
    }
    if matches!(kind, b']' | b'P' | b'_' | b'^') {
        let end = if kind == b']' {
            osc_end(bytes, start + 2)
        } else {
            string_control_end(bytes, start + 2)
        };
        let sequence = &bytes[start..end];
        let complete =
            sequence.ends_with(b"\x1b\\") || (kind == b']' && sequence.ends_with(b"\x07"));
        return EscapeSequence { end, complete };
    }
    // 2. 【终端】【字符集控制】ESC 中间字节之后必须有最终字节，不能把下一条 ESC 一并吞掉
    let mut end = start + 1;
    while bytes
        .get(end)
        .is_some_and(|byte| (0x20..=0x2f).contains(byte))
    {
        end += 1;
    }
    let complete = bytes
        .get(end)
        .is_some_and(|byte| (0x30..=0x7e).contains(byte));
    if complete {
        end += 1;
    }
    EscapeSequence { end, complete }
}

/// 查找 CSI 序列结束位置。
///
/// 参数:
/// - `bytes`: 原始字节
/// - `index`: CSI 参数起始位置
///
/// 返回:
/// - CSI 安全结束位置与闭合状态；非法字节保留给后续文本扫描
fn csi_end(bytes: &[u8], mut index: usize) -> EscapeSequence {
    while index < bytes.len() {
        let byte = bytes[index];
        if (0x40..=0x7e).contains(&byte) {
            return EscapeSequence {
                end: index + 1,
                complete: true,
            };
        }
        if !(0x20..=0x3f).contains(&byte) {
            break;
        }
        index += 1;
    }
    EscapeSequence {
        end: index,
        complete: false,
    }
}

/// 查找 OSC 序列结束位置。
///
/// 参数:
/// - `bytes`: 原始字节
/// - `index`: OSC 内容起始位置
///
/// 返回:
/// - BEL 或 ST 后的字节位置
fn osc_end(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() {
        if bytes[index] == 0x07 {
            return index + 1;
        }
        if bytes[index] == 0x1b && bytes.get(index + 1) == Some(&b'\\') {
            return index + 2;
        }
        index += 1;
    }
    bytes.len()
}

/// 查找 DCS、APC 等字符串控制序列结束位置。
///
/// 参数:
/// - `bytes`: 原始字节
/// - `index`: 控制内容起始位置
///
/// 返回:
/// - ST 后的字节位置
fn string_control_end(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() {
        if bytes[index] == 0x1b && bytes.get(index + 1) == Some(&b'\\') {
            return index + 2;
        }
        index += 1;
    }
    bytes.len()
}

#[cfg(test)]
mod escape_tests {
    use super::*;

    /// 【终端】【分片闭合回归】控制前缀未闭合时不能输出，最终字节、BEL 和 ST 均能正确闭合。
    /// 参数：无；返回：无，闭合状态或后续可见字符边界错误时断言失败。
    #[test]
    fn incomplete_control_prefixes_are_not_emittable() {
        for sequence in [
            "\x1b[38;2;170;232;246m",
            "\x1b]8;;https://example.invalid\x1b\\",
            "\x1b]title\x07",
            "\x1b_Gpayload\x1b\\",
            "\x1bPpayload\x1b\\",
            "\x1b(B",
        ] {
            for end in 1..sequence.len() {
                assert!(
                    !scan_escape_sequence(&sequence[..end], 0).complete,
                    "{:?}",
                    &sequence[..end]
                );
            }
            let scanned = scan_escape_sequence(&format!("{sequence}tail"), 0);
            assert!(scanned.complete);
            assert_eq!(scanned.end, sequence.len());
        }
        assert_eq!(
            scan_escape_sequence("\x1b[31\u{fffd}", 0),
            EscapeSequence {
                end: 4,
                complete: false
            }
        );
        assert_eq!(scan_escape_sequence("\x1b\x1b[0m", 0).end, 1);
    }

    /// 【终端】【UTF-8 边界回归】ESC 后的多字节字符不是转义类型，不得跳进字符编码内部。
    /// 参数：无；返回：无，扫描位置越界或吞掉可见字符时断言失败。
    #[test]
    fn unknown_escape_preserves_multibyte_follower() {
        for character in ['\u{fffd}', '中', 'é', '\u{1d11e}', '\u{0301}'] {
            let text = format!("prefix\x1b{character}tail");
            let start = "prefix".len();
            let end = escape_sequence_end(&text, start);
            assert_eq!(end, start + 1, "{text:?}");
            assert_eq!(&text[end..], format!("{character}tail"));
        }
    }

    /// 【终端】【截断边界回归】完整或截断控制序列只能返回合法 UTF-8 字符边界。
    /// 参数：无；返回：无，任意序列前缀产生非法切片位置时断言失败。
    #[test]
    fn every_sequence_prefix_ends_at_a_character_boundary() {
        for text in [
            "前\x1b\u{fffd}后",
            "\x1b[31m中文\x1b[0m",
            "\x1b[31\u{fffd}",
            "\x1b]caption中文\x07后",
            "\x1b]caption中文\x1b\\后",
            "\x1b_Gpayload中文\x1b\\后",
            "\x1bPpayloadé\x1b\\后",
            "\x1b^payload\u{1d11e}\x1b\\后",
            "\x1b(中文\x1b\\",
        ] {
            for limit in (0..=text.len()).filter(|index| text.is_char_boundary(*index)) {
                let prefix = &text[..limit];
                for (start, character) in prefix.char_indices() {
                    if character != '\x1b' {
                        continue;
                    }
                    let end = escape_sequence_end(prefix, start);
                    assert!(
                        end > start && end <= prefix.len(),
                        "{prefix:?}: {start}..{end}"
                    );
                    assert!(prefix.is_char_boundary(end), "{prefix:?}: {start}..{end}");
                }
            }
        }
    }

    #[test]
    fn consumes_kitty_payload_as_one_sequence() {
        let text = "\x1b_Gf=100,a=T;abcdefghijklmnopqrstuvwxyz\x1b\\tail";

        assert_eq!(
            &text[..escape_sequence_end(text, 0)],
            "\x1b_Gf=100,a=T;abcdefghijklmnopqrstuvwxyz\x1b\\"
        );
    }

    #[test]
    fn consumes_iterm_payload_until_bell() {
        let text = "\x1b]1337;File=inline=1:abcdef\x07tail";

        assert_eq!(
            &text[..escape_sequence_end(text, 0)],
            "\x1b]1337;File=inline=1:abcdef\x07"
        );
    }
}
