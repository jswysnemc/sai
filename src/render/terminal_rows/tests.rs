use super::*;

/// 【终端】【逐行绘制】缩短正文后清理行尾，但不能清空整屏或重写相同行。
/// 参数: 无
/// 返回: 无，顺序或绘制范围错误时断言失败
#[test]
fn paints_only_changes_before_erasing_stale_suffixes() {
    let previous = vec!["stable".into(), "old long value".into()];
    let current = vec!["stable".into(), "new".into()];
    let mut output = Vec::new();
    paint_changed_rows(&mut output, 3, 40, &current, Some(&previous)).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(!output.contains("stable"));
    assert!(!output.contains("\x1b[2J") && !output.contains("\x1b[2K"));
    assert!(output.find("new").unwrap() < output.find("\x1b[K").unwrap());
}

/// 【终端】【逐行绘制】满宽文本不能在最后一列执行清行，否则最后一个字符会消失。
/// 参数: 无
/// 返回: 无，覆盖最后一列时断言失败
#[test]
fn full_width_row_does_not_erase_its_last_character() {
    let mut output = Vec::new();
    paint_changed_rows(&mut output, 0, 4, &["abcd".into()], None).unwrap();
    assert!(!String::from_utf8(output).unwrap().contains("\x1b[K"));
}

/// 【终端】【逐行绘制】满宽行在恢复自动换行前把光标钉回行尾，取消延迟换行。
/// 参数: 无
/// 返回: 无，光标定位晚于恢复换行时断言失败
#[test]
fn full_width_row_parks_cursor_before_wrap_is_restored() {
    let mut output = Vec::new();
    paint_changed_rows(&mut output, 0, 4, &["abcd".into()], None).unwrap();
    let output = String::from_utf8(output).unwrap();
    let park = output
        .find("\x1b[1;4H")
        .expect("cursor parked on last cell");
    let restore = output.find("\x1b[?7h").expect("autowrap restored");
    assert!(park < restore);
}

/// 【终端】【行内绘制】同样式追加只写新增列，不重复旧正文。
#[test]
fn append_writes_only_new_cells() {
    let mut output = Vec::new();
    paint_changed_rows(
        &mut output,
        0,
        40,
        &["正在检查当前仓库".into()],
        Some(&["正在检查".into()]),
    )
    .unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(!output.contains("正在检查"));
    assert!(output.contains("当前仓库"));
    assert!(output.contains("\x1b[1;9H"));
}

/// 【终端】【行内绘制】样式变化、字符簇拆改和行缩短回退完整覆盖。
#[test]
fn style_grapheme_and_removal_fall_back_to_full_row() {
    for (old, new) in [
        ("\x1b[32mold\x1b[0m", "\x1b[31mold new\x1b[0m"),
        ("e", "e\u{301} x"),
        ("long text", "short"),
    ] {
        let mut output = Vec::new();
        paint_changed_rows(&mut output, 0, 40, &[new.into()], Some(&[old.into()])).unwrap();
        assert!(String::from_utf8(output)
            .unwrap()
            .starts_with("\x1b[?7l\x1b[1;1H"));
    }
}

/// 【终端】【样式追加】新增文本继承实际样式，不输出用于比较的旧文字。
/// 参数：无；返回：无，追加列数或样式不一致时断言失败。
#[test]
fn styled_append_replays_only_sgr_before_suffix() {
    for (old, new, column, suffix, style) in [
        (
            "\x1b[32mold\x1b[0m",
            "\x1b[32mold new\x1b[0m",
            4,
            " new",
            "\x1b[32m",
        ),
        (
            "\x1b[1m\x1b[38;2;0;160;90mold\x1b[0m",
            "\x1b[1m\x1b[38;2;0;160;90mold new\x1b[0m",
            4,
            " new",
            "\x1b[1m\x1b[38;2;0;160;90m",
        ),
        ("old", "old\x1b[34m new\x1b[0m", 4, " new", "\x1b[34m"),
        ("e\u{301}", "e\u{301}界", 2, "界", ""),
    ] {
        let mut output = Vec::new();
        paint_changed_rows(&mut output, 2, 40, &[new.into()], Some(&[old.into()])).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(
            output.starts_with(&format!("\x1b[?7l\x1b[3;{column}H\x1b[0m{style}{suffix}")),
            "{output:?}"
        );
        assert!(!output.contains("old"));
    }
}

/// 【终端】【满宽追加】ASCII 和双列字符到达行尾后，恢复自动换行前固定光标。
/// 参数：无；返回：无，末列被清除或延迟换行未取消时断言失败。
#[test]
fn full_width_append_parks_without_erasing_last_cell() {
    for (old, new, cols) in [("abc", "abcd", 4), ("中", "中文", 4)] {
        let mut output = Vec::new();
        paint_changed_rows(&mut output, 0, cols, &[new.into()], Some(&[old.into()])).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(!output.contains("\x1b[K"));
        assert!(output.ends_with("\x1b[1;4H\x1b[?7h"));
    }
}

/// 【终端】【控制序列回退】光标控制、链接和跨样式字符簇不能按普通追加定位。
/// 参数：无；返回：无，不安全输入未回退整行时断言失败。
#[test]
fn controls_and_split_graphemes_disable_append() {
    for (old, new) in [
        ("a", "a\tmore"),
        ("a", "a\rmore"),
        ("a", "a\x1b[2Cmore"),
        ("a", "a\x1b]8;;https://example.com\x1b\\more\x1b]8;;\x1b\\"),
        ("e", "e\x1b[31m\u{301}x"),
        ("abc", "abcdef"),
    ] {
        let mut output = Vec::new();
        paint_changed_rows(&mut output, 0, 5, &[new.into()], Some(&[old.into()])).unwrap();
        assert!(String::from_utf8(output)
            .unwrap()
            .starts_with("\x1b[?7l\x1b[1;1H"));
    }
    for text in ["\x1b", "\x1b[", "\x1b[31", "\x1b[31x"] {
        assert!(styled_row::parse(text).is_none());
    }
}

/// 【终端】【输出量回归】长行只写新增片段，终端输出不随稳定前缀增长。
/// 参数：无；返回：无，输出仍包含稳定前缀时断言失败。
#[test]
fn appended_output_is_independent_of_prefix_length() {
    let old = "x".repeat(300);
    let new = format!("{old} tail");
    let mut output = Vec::new();
    paint_changed_rows(&mut output, 0, 400, &[new], Some(&[old])).unwrap();
    assert!(output.len() < 60);
    assert!(String::from_utf8(output).unwrap().contains(" tail"));
}

/// 【终端】【填充追加回归】全屏定宽行只替换填充空格，保留原列上的概览轨道。
/// 参数：无；返回：无，旧文字或未变化轨道重新输出时断言失败。
#[test]
fn append_into_padding_leaves_the_fixed_rail_untouched() {
    for (old, new, column, suffix) in [
        (
            "old       \x1b[2m┃\x1b[0m",
            "old new   \x1b[2m┃\x1b[0m",
            5,
            "new",
        ),
        (
            "中        \x1b[2m┃\x1b[0m",
            "中文      \x1b[2m┃\x1b[0m",
            3,
            "文",
        ),
    ] {
        for cols in [11, 40] {
            let mut output = Vec::new();
            paint_changed_rows(&mut output, 0, cols, &[new.into()], Some(&[old.into()])).unwrap();
            let output = String::from_utf8(output).unwrap();
            assert!(
                output.starts_with(&format!("\x1b[?7l\x1b[1;{column}H\x1b[0m{suffix}")),
                "{output:?}"
            );
            assert!(!output.contains("old") && !output.contains('┃'));
            assert!(!output.contains("\x1b[K"));
        }
    }
}
