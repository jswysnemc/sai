use super::{
    align_cli_stream_block, align_cli_text_delta, align_to_guide_column,
    align_to_guide_column_with_width, clear_right_margin, indent_diff_for_cli,
    indent_diff_for_transcript, wrap_cli_stream_block_with_width, CONTENT_LEFT_INDENT,
};

/// 【终端】【视觉引导测试】脉冲圆点始终停在引导列，窄终端按可用空间压缩间隔。
/// 参数：无；返回：无。
#[test]
fn animated_guides_stay_aligned_at_every_supported_width() {
    use crate::render::activity_animation::{render_activity_line, strip_ansi_for_test};

    for frame in [0, 12, 31, 62] {
        let line = render_activity_line("Working", "", frame);
        for (width, expected) in [(2, "● Working"), (1, "●Working"), (0, "Working")] {
            let aligned = align_to_guide_column_with_width(&line, width);
            assert_eq!(strip_ansi_for_test(&aligned), expected);
        }
        assert_eq!(align_cli_stream_block(&line), line);
    }
}

/// 引导符号保留在左侧，普通正文与续行位于右侧。
#[test]
fn aligns_symbols_and_content_on_opposite_sides() {
    assert_eq!(align_to_guide_column("a"), "  a");
    assert_eq!(align_to_guide_column("  continuation"), "  continuation");
    assert_eq!(
        align_to_guide_column("\x1b[36m●\x1b[0m input"),
        "\x1b[36m●\x1b[0m input"
    );
    assert_eq!(align_to_guide_column("• tool"), "● tool");
    assert_eq!(align_to_guide_column("◦ Thinking"), "○ Thinking");
    // 通栏 turn 线保持顶格；短 MD 线（正文列内再左右内收）收入引导区右侧
    let full = crate::render::markdown_blocks::horizontal_rule_width();
    let turn = "─".repeat(full);
    let aligned_turn = align_to_guide_column(&turn);
    assert_eq!(
        crate::render::activity_animation::strip_ansi_for_test(&aligned_turn),
        turn
    );
    let content = full.saturating_sub(CONTENT_LEFT_INDENT).max(1);
    let inset =
        crate::render::markdown_blocks::MARKDOWN_HR_SIDE_INSET.min(content.saturating_sub(1) / 2);
    let md = "─".repeat(content.saturating_sub(inset.saturating_mul(2)).max(1));
    assert_eq!(
        align_to_guide_column(&md),
        format!("{}{md}", " ".repeat(CONTENT_LEFT_INDENT))
    );
    // 已带正文缩进的线不再二次缩进
    assert_eq!(align_to_guide_column("  ────────"), "  ────────");
    // 失败叉号与提示引导符同样悬挂在引导线列，不与正文混排
    assert_eq!(
        align_to_guide_column("\x1b[31m✗ 本轮失败\x1b[0m"),
        "\x1b[31m● 本轮失败\x1b[0m"
    );
    assert_eq!(
        align_to_guide_column("\x1b[2m› 已切换模型\x1b[0m"),
        "\x1b[2m○ 已切换模型\x1b[0m"
    );
    assert_eq!(align_to_guide_column(" diff"), "   diff");
    assert_eq!(
        align_to_guide_column("\x1b[48;5;22m+line\x1b[K\x1b[0m"),
        "   \x1b[48;5;22m+line\x1b[K\x1b[0m"
    );
    assert_eq!(indent_diff_for_transcript("a\nb"), " a\n b");
    assert_eq!(indent_diff_for_cli("a\nb"), "   a\n   b");
    // diff 标题行的引导符保持顶格，正文色块才内收
    assert_eq!(
        indent_diff_for_transcript("• Added a.txt (+1 -0)\nline"),
        "• Added a.txt (+1 -0)\n line"
    );
    assert_eq!(align_to_guide_column(" • Added a.txt"), "● Added a.txt");
}

/// 【终端】【diff 对齐】验证同一 diff 块内三类行的正文落在同一列。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn diff_lines_share_one_content_column() {
    // 变更落在第 19 行附近，使行号为 2 位而列宽为 3，暴露填充空格位置差异
    let mut body = (1..=22)
        .map(|number| format!("line{number}"))
        .collect::<Vec<_>>();
    body[18] = "  \"npm:pi-markdown-preview\"".to_string();
    let rendered = render_transcript_diff(
        &body.join("\n"),
        "  \"npm:pi-markdown-preview\"",
        "  \"npm:pi-markdown-preview\",\n  \"npm:pi-readseek\"",
    );

    let columns = diff_body_columns(&rendered);
    assert!(columns.len() >= 5, "样例应同时包含上下文行与增删行");
    assert!(
        columns.windows(2).all(|pair| pair[0] == pair[1]),
        "diff 块内所有行的正文必须落在同一列: {columns:?}"
    );
}

/// 【终端】【diff 对齐】验证行号位数跨越十位时正文列不漂移。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn diff_content_column_is_stable_across_line_number_widths() {
    // 上下文覆盖第 9 到第 11 行，行号同时出现 1 位与 2 位
    let body = (1..=14)
        .map(|number| format!("line{number}"))
        .collect::<Vec<_>>()
        .join("\n");
    let rendered = render_transcript_diff(&body, "line10", "changed10");

    let columns = diff_body_columns(&rendered);
    assert!(columns.len() >= 5, "样例应跨越行号位数变化");
    assert!(
        columns.windows(2).all(|pair| pair[0] == pair[1]),
        "行号位数变化不应改变正文列: {columns:?}"
    );
}

/// 按 transcript 路径渲染一次 diff。
///
/// 参数:
/// - `original`: 变更前的文件内容
/// - `hunk`: patch hunk 文本
///
/// 返回:
/// - 已补齐块缩进的 diff 文本
fn render_transcript_diff(original: &str, old_string: &str, new_string: &str) -> String {
    let temp = tempfile::tempdir().expect("临时目录");
    let path = temp.path().join("settings.json");
    std::fs::write(&path, format!("{original}\n")).expect("写入样例文件");
    let arguments = serde_json::json!({
        "path": path.display().to_string(),
        "old_string": old_string,
        "new_string": new_string
    })
    .to_string();
    let preview =
        crate::render::edit_diff::preview_from_arguments(&arguments).expect("diff 应能构建");
    crate::render::edit_diff::render_patch_preview_for_transcript(&preview)
}

/// 提取 diff 正文行经引导对齐后的首内容列。
///
/// 参数:
/// - `rendered`: 渲染完成的 diff 文本块
///
/// 返回:
/// - 每个正文行的首内容列，跳过标题行
fn diff_body_columns(rendered: &str) -> Vec<usize> {
    rendered
        .lines()
        .filter(|line| line.contains("\x1b[K"))
        .map(|line| first_content_column(&align_to_guide_column(line)))
        .collect()
}

/// 计算行首到首个可见非空格字符的显示列数。
///
/// 参数:
/// - `text`: 带 ANSI 样式的终端行
///
/// 返回:
/// - 首个可见非空格字符所在列
fn first_content_column(text: &str) -> usize {
    let mut column = 0usize;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
            continue;
        }
        if ch != ' ' {
            return column;
        }
        column += 1;
    }
    column
}

/// 【终端】【响应式引导测试】验证窄终端压缩间隔或移除符号后不吞正文。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn compacts_visual_guide_for_narrow_terminals() {
    let marker = "\x1b[2m\x1b[36m•\x1b[0m answer";

    assert_eq!(align_to_guide_column_with_width("answer", 1), " answer");
    assert_eq!(
        align_to_guide_column_with_width("  continuation", 1),
        " continuation"
    );
    assert_eq!(
        align_to_guide_column_with_width(marker, 1),
        "\x1b[2m\x1b[36m●\x1b[0manswer"
    );
    assert_eq!(align_to_guide_column_with_width(marker, 0), "answer");
}

/// 右侧边距序列必须清除目标列数并恢复原光标。
///
/// 用 DECSC/DECRC 而非 SCOSC/SCOR：后者在部分终端上被静默忽略。
#[test]
fn right_margin_clear_preserves_cursor_position() {
    let sequence = clear_right_margin(3);
    assert!(sequence.starts_with("\x1b7"));
    assert!(sequence.contains("\x1b[2D\x1b[3X"));
    assert!(sequence.ends_with("\x1b8"));
}

/// 【终端】【CLI 布局】验证普通正文右移，引导符号保留在左侧。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn aligns_cli_body_without_moving_leading_markers() {
    let block = "answer\n\x1b[36m•\x1b[0m tool\n";

    assert_eq!(
        align_cli_stream_block(block),
        "  answer\n\x1b[36m●\x1b[0m tool\n"
    );
}

/// 【终端】【CLI 布局】验证表格重绘控制先执行，再缩进新正文。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn preserves_cursor_controls_before_aligned_content() {
    let block = "\x1b[1A\r\x1b[2Ktable\n";

    assert_eq!(align_cli_stream_block(block), "\x1b[1A\r\x1b[2K  table\n");
}

/// 【终端】【CLI 布局测试】验证纯文本分片只在物理行起点缩进。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn aligns_fragmented_plain_text_without_midline_spaces() {
    let mut at_line_start = true;

    assert_eq!(
        align_cli_text_delta("partial", &mut at_line_start),
        "  partial"
    );
    assert!(!at_line_start);
    assert_eq!(
        align_cli_text_delta(" answer\nnext\n", &mut at_line_start),
        " answer\n  next\n"
    );
    assert!(at_line_start);
}

/// 【终端】【CLI 换行】验证长行续行显式回到下一物理行。
#[test]
fn wraps_long_cli_lines_before_the_guide_alignment() {
    let long = "a".repeat(110);
    let wrapped = wrap_cli_stream_block_with_width(&long, 98);
    assert!(wrapped.contains('\n'));
    assert!(wrapped.lines().all(|line| line.chars().count() <= 98));
}
