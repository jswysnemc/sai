/// 【终端】【命令摘要】展示脚本实际内容，避免不同命令都只显示 bash -c。
/// 参数：`value` 为完整或流式命令；返回最多 48 显示列的摘要。
pub(super) fn command_summary(value: String) -> String {
    let lines = value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let Some(first) = lines.first() else {
        return String::new();
    };
    // 1. 单独的 shell 包装行没有辨识度，提取其后脚本；不执行或解释脚本
    let wrapper = matches!(
        *first,
        "bash -c '"
            | "bash -lc '"
            | "sh -c '"
            | "zsh -c '"
            | "bash -c \""
            | "bash -lc \""
            | "sh -c \""
            | "zsh -c \""
    );
    let body = if wrapper && lines.len() > 1 {
        &lines[1..]
    } else {
        &lines[..]
    };
    let first_body = body[0];
    // 2. cd、数组开头同样补上后续内容，便于区分同目录下不同批次
    let setup = first_body.starts_with("cd ") || first_body.ends_with("=(");
    let text = if wrapper || setup {
        body.iter()
            .copied()
            .filter(|line| !matches!(*line, "'" | "\""))
            .take(6)
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        first_body.to_string()
    };
    super::compact_text(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【命令摘要测试】相同包装脚本显示各自操作对象；无参数、无返回值。
    #[test]
    fn distinguishes_multiline_shell_commands() {
        let first = command_summary("bash -c '\ncat first/PKGBUILD\n'".into());
        let second = command_summary("bash -c '\ncat second/PKGBUILD\n'".into());
        assert_eq!(first, "cat first/PKGBUILD");
        assert_eq!(second, "cat second/PKGBUILD");
        assert_eq!(
            command_summary("cd /tmp/review\ncat PKGBUILD".into()),
            "cd /tmp/review cat PKGBUILD"
        );
        assert_eq!(
            command_summary("cargo test\ncargo build".into()),
            "cargo test"
        );
        assert_eq!(command_summary("bash -c '".into()), "bash -c '");
    }
}
