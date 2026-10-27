/// 【终端】【悬停按钮】悬停时用单列大于号表示可展开，不添加下划线或改变文字位置。
/// 参数: `line` 为 ANSI 标题，`expanded` 为展开状态；返回: 临时替换引导符的标题。
pub(in crate::cli::repl_runtime) fn render(line: &str, _expanded: bool) -> String {
    let replaced = crate::render::guide_symbols::replace(line, ">");
    if replaced != line {
        return replaced;
    }
    // 1. 【终端】【文字按钮】无图标的页脚使用已有缩进列，不覆盖行号或正文
    let mut index = 0;
    let mut spaces = Vec::new();
    while index < line.len() {
        let ch = line[index..].chars().next().unwrap();
        if ch == '\x1b' {
            index = crate::render::terminal_image::escape_sequence_end(line, index).max(index + 1);
        } else if ch == ' ' {
            spaces.push(index);
            index += 1;
        } else {
            break;
        }
    }
    if let Some(&index) = spaces.get(spaces.len().saturating_sub(2)) {
        format!("{}>{}", &line[..index], &line[index + 1..])
    } else {
        line.to_string()
    }
}

#[cfg(test)]
mod tests {
    /// 【终端】【悬停回归】大于号只占原图标一列，不产生下划线；无参数和返回值。
    #[test]
    fn hover_uses_disclosure_marker_without_changing_width() {
        for icon in [crate::render::guide_symbols::READ, "◇", "▸", "▾", "─"] {
            let source = format!("\x1b[2m{icon} title\x1b[0m   ");
            let result = super::render(&source, true);
            assert!(!result.contains("\x1b[4m"));
            assert_eq!(
                crate::render::activity_animation::strip_ansi_for_test(&result),
                "> title   "
            );
        }
        assert_eq!(super::render("  1-5/10 Resume", false), "> 1-5/10 Resume");
    }
}
