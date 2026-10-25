/// 【终端】【悬停按钮】仅替换可展开标题的首个引导符号，不修改正文、着色或占用列数。
/// 参数: `line` 为完整 ANSI 行，`expanded` 为当前展开状态；返回: 带展开或收起按钮的标题。
pub(super) fn render(line: &str, expanded: bool) -> String {
    let mut offset = 0;
    while offset < line.len() {
        if line.as_bytes()[offset] == b'\x1b' {
            offset =
                crate::render::terminal_image::escape_sequence_end(line, offset).max(offset + 1);
            continue;
        }
        let Some(character) = line[offset..].chars().next() else {
            break;
        };
        if character == ' ' {
            offset += 1;
            continue;
        }
        if matches!(
            character,
            '●' | '•'
                | '·'
                | '$'
                | '◇'
                | '▤'
                | '▸'
                | '▾'
                | '⌄'
                | '›'
                | '○'
                | '◦'
                | '◐'
                | '◓'
                | '◑'
                | '◒'
                | '✗'
        ) {
            return format!(
                "{}{}{}",
                &line[..offset],
                if expanded { "▾" } else { "▸" },
                &line[offset + character.len_utf8()..]
            );
        }
        break;
    }
    line.to_string()
}
