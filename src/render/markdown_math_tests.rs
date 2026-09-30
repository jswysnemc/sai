use super::*;

/// 用测试替身渲染完整 Markdown，返回最终文本。
fn render_stable(markdown: &str) -> String {
    crate::render::asset_block::set_test_stub(true);
    let mut renderer = MarkdownStreamRenderer::new_stable();
    let mut output = renderer.push(markdown);
    output.push_str(&renderer.flush());
    crate::render::asset_block::set_test_stub(false);
    output
}

/// 验证行内公式后文字留在同一行，不被强制换行。
#[test]
fn inline_formula_keeps_following_text_on_the_same_line() {
    let output = render_stable("质能等价公式 $E=mc^2$，欧拉公式 $e^{ix}$。\n");
    let line = output
        .lines()
        .find(|line| line.contains("质能等价公式"))
        .expect("paragraph line");
    assert!(line.contains("，欧拉公式"), "{output:?}");
    assert!(line.contains('。'), "{output:?}");
    assert_eq!(line.matches("[inline math rendering skipped]").count(), 2);
}

/// 验证独占一行的 `$$…$$` 按块级公式出图，而不是走行内图片。
#[test]
fn whole_line_double_dollar_renders_as_display_block() {
    let output = render_stable("1. 欧拉恒等式\n$$ e^{i\\pi} + 1 = 0 $$\n2. 下一项\n");
    assert!(output.contains("[asset rendering skipped]"), "{output:?}");
    assert!(
        !output.contains("[inline math rendering skipped]"),
        "{output:?}"
    );
    assert!(output.contains("下一项"));
}

/// 验证定界符与首尾公式同行的多行公式整体出图，不再报空公式。
#[test]
fn aligned_block_with_inline_delimiters_renders_once() {
    let output = render_stable(
        "5. 麦克斯韦方程组\n$$\\begin{aligned}\n\\nabla \\cdot \\mathbf{E} &= \\frac{\\rho}{\\varepsilon_0} \\\\\n\\nabla \\cdot \\mathbf{B} &= 0\n\\end{aligned}$$\n结束\n",
    );
    assert!(!output.contains("render failed"), "{output:?}");
    assert!(!output.contains("content is empty"), "{output:?}");
    assert_eq!(
        output.matches("[asset rendering skipped]").count(),
        1,
        "{output:?}"
    );
    assert!(output.contains("结束"));
}

/// 验证空的 `$$` 与 `$ $` 不会被当成公式报错。
#[test]
fn empty_delimiters_are_kept_as_text() {
    let output = render_inline("价格 $ $ 与 $$$$ 都不是公式");
    assert!(!output.contains("render failed"), "{output:?}");
    assert!(output.contains("都不是公式"));
}

/// 验证 Kitty 终端的真实渲染：行内公式不产生换行，同行重复公式各占一个放置。
#[test]
fn kitty_inline_formulas_stay_inline_with_distinct_placements() {
    crate::render::terminal_image::test_override::set(Some(true), Some(false), Some(false));
    let output = render_inline("设 $x$ 为实数，则 $x^2 \\ge 0$，且 $x$ 可以取负值");
    crate::render::terminal_image::test_override::set(None, None, None);
    assert!(!output.contains('\n'), "{output:?}");
    assert!(output.contains("可以取负值"));
    let placements = output
        .split("\x1b_G")
        .skip(1)
        .filter_map(|chunk| chunk.split(",p=").nth(1))
        .filter_map(|rest| rest.split(',').next())
        .collect::<Vec<_>>();
    assert_eq!(placements.len(), 3, "{output:?}");
    assert_ne!(placements[0], placements[2], "{output:?}");
}

/// 验证块级公式与上下正文各隔一行，已有空行时不重复叠加。
#[test]
fn display_math_is_separated_from_surrounding_text() {
    let tight = render_stable("说明 $a$：\n$$ x=1 $$\n下一段\n");
    assert!(
        tight.contains("说明 [inline math rendering skipped]：\n\n"),
        "{tight:?}"
    );
    assert!(tight.contains("\n\n下一段"), "{tight:?}");
    let spaced = render_stable("说明\n\n$$ x=1 $$\n\n下一段\n");
    assert!(!spaced.contains("\n\n\n"), "{spaced:?}");
    let multi = render_stable("说明\n$$\\begin{aligned}\na&=1\n\\end{aligned}$$\n下一段\n");
    assert!(multi.contains("说明\n\n"), "{multi:?}");
    assert!(multi.contains("\n\n下一段"), "{multi:?}");
}
