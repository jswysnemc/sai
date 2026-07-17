#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct MessageInputFlags {
    pub(crate) message: String,
    pub(crate) clipb: bool,
    pub(crate) web_search: bool,
}

/// 从消息开头提取自然语言入口标志。
///
/// 参数:
/// - `parts`: 原始消息参数
/// - `clipb`: clap 已解析的剪贴板标志
/// - `web_search`: clap 已解析的网络搜索标志
///
/// 返回:
/// - 归一化后的消息和入口标志
pub(crate) fn parse_message_input_flags(
    parts: Vec<String>,
    clipb: bool,
    web_search: bool,
) -> MessageInputFlags {
    let mut has_clipboard = clipb;
    let mut has_web_search = web_search;
    let mut first_message_index = 0usize;
    for part in &parts {
        match part.as_str() {
            "-c" | "--clipb" => has_clipboard = true,
            "-w" | "--web" | "--web-search" => has_web_search = true,
            _ => break,
        }
        first_message_index += 1;
    }
    MessageInputFlags {
        message: parts[first_message_index..].join(" ").trim().to_string(),
        clipb: has_clipboard,
        web_search: has_web_search,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_leading_clipboard_flag() {
        let input = parse_message_input_flags(
            vec!["-c".to_string(), "总结".to_string(), "这段".to_string()],
            false,
            false,
        );

        assert!(input.clipb);
        assert!(!input.web_search);
        assert_eq!(input.message, "总结 这段");
    }

    #[test]
    fn extracts_leading_web_search_flag() {
        let input =
            parse_message_input_flags(vec!["-w".to_string(), "搜索".to_string()], false, false);

        assert!(!input.clipb);
        assert!(input.web_search);
        assert_eq!(input.message, "搜索");
    }

    #[test]
    fn extracts_multiple_leading_flags() {
        let input = parse_message_input_flags(
            vec!["-w".to_string(), "-c".to_string(), "总结".to_string()],
            false,
            false,
        );

        assert!(input.clipb);
        assert!(input.web_search);
        assert_eq!(input.message, "总结");
    }

    #[test]
    fn keeps_non_leading_clipboard_flag_literal() {
        let input = parse_message_input_flags(
            vec!["解释".to_string(), "-c".to_string(), "参数".to_string()],
            false,
            false,
        );

        assert!(!input.clipb);
        assert_eq!(input.message, "解释 -c 参数");
    }

    #[test]
    fn keeps_trailing_clipboard_flag_literal() {
        let input =
            parse_message_input_flags(vec!["总结".to_string(), "-c".to_string()], false, false);

        assert!(!input.clipb);
        assert_eq!(input.message, "总结 -c");
    }
}
