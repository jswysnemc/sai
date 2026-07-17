/// 返回从指定位置开始的完整终端转义序列结束位置。
///
/// 支持 CSI、OSC、DCS 与 APC，保证图片协议载荷不会被文本换行器拆分。
///
/// 参数:
/// - `text`: 原始终端文本
/// - `start`: ESC 所在字节位置
///
/// 返回:
/// - 转义序列后第一个字节位置
pub(crate) fn escape_sequence_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&0x1b) {
        return start;
    }
    let Some(kind) = bytes.get(start + 1).copied() else {
        return start + 1;
    };
    match kind {
        b'[' => csi_end(bytes, start + 2),
        b']' => osc_end(bytes, start + 2),
        b'P' | b'_' | b'^' => string_control_end(bytes, start + 2),
        _ => (start + 2).min(bytes.len()),
    }
}

/// 查找 CSI 序列结束位置。
///
/// 参数:
/// - `bytes`: 原始字节
/// - `index`: CSI 参数起始位置
///
/// 返回:
/// - CSI 结束后的字节位置
fn csi_end(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() {
        let byte = bytes[index];
        index += 1;
        if (0x40..=0x7e).contains(&byte) {
            break;
        }
    }
    index
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
