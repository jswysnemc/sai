/// 为工具状态添加终端颜色。
///
/// 参数:
/// - `status`: 工具状态，常见取值为 arg、run、ok 或 err
///
/// 返回:
/// - 带 ANSI 颜色的状态文本，未知状态返回原文本
pub(crate) fn color_status(status: &str) -> String {
    match status {
        "ok" => "\x1b[32mok\x1b[0m".to_string(),
        "err" => "\x1b[31merr\x1b[0m".to_string(),
        "run" => "\x1b[33mrun\x1b[0m".to_string(),
        "arg" => "\x1b[36m...\x1b[0m".to_string(),
        value => value.to_string(),
    }
}

/// 为运行中状态标签添加终端颜色。
///
/// 参数:
/// - `label`: 本地化后的运行中标签
///
/// 返回:
/// - 带 ANSI 颜色的运行中标签
pub(crate) fn color_running(label: &str) -> String {
    format!("\x1b[33m{label}\x1b[0m")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_status_styles_known_values() {
        assert_eq!(color_status("ok"), "\x1b[32mok\x1b[0m");
        assert_eq!(color_status("err"), "\x1b[31merr\x1b[0m");
        assert_eq!(color_status("run"), "\x1b[33mrun\x1b[0m");
        assert_eq!(color_status("arg"), "\x1b[36m...\x1b[0m");
        assert_eq!(color_status("custom"), "custom");
    }

    #[test]
    fn color_running_styles_localized_label() {
        assert_eq!(color_running("running"), "\x1b[33mrunning\x1b[0m");
        assert_eq!(color_running("运行中"), "\x1b[33m运行中\x1b[0m");
    }
}
