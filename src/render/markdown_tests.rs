use super::*;
use crate::render::style::{
    BOLD_STYLE, CODE_BLOCK_FRAME_STYLE, CODE_FUNCTION_STYLE, CODE_KEYWORD_STYLE, CODE_TOKEN_RESET,
    HEADER_STYLE, IMAGE_STYLE, INLINE_CODE_STYLE, ITALIC_STYLE, LINK_LABEL_STYLE, PRIMARY_STYLE,
    RESET, STRIKE_STYLE, TERTIARY_STYLE, URL_STYLE,
};
use crate::render::table;
use std::sync::Mutex;

static ASSET_STUB_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn streams_only_complete_lines() {
    let mut renderer = MarkdownStreamRenderer::new();
    assert_eq!(renderer.push("**bo"), "");
    assert_eq!(
        renderer.push("ld**\n"),
        format!("{BOLD_STYLE}bold{RESET}\n")
    );
}

#[test]
fn flushes_partial_final_line() {
    let mut renderer = MarkdownStreamRenderer::new();
    assert_eq!(renderer.push("# Title"), "");
    assert_eq!(renderer.flush(), format!("{HEADER_STYLE}# Title{RESET}\n"));
}

#[test]
fn headings_use_one_color_and_distinct_prefix_lengths() {
    assert_eq!(
        render_markdown_line("# One"),
        format!("{HEADER_STYLE}# One{RESET}")
    );
    assert_eq!(
        render_markdown_line("## Two"),
        format!("{HEADER_STYLE}## Two{RESET}")
    );
    assert_eq!(
        render_markdown_line("### Three"),
        format!("{HEADER_STYLE}### Three{RESET}")
    );
    assert_eq!(
        render_markdown_line("###### Six"),
        format!("{HEADER_STYLE}###### Six{RESET}")
    );
}

#[test]
fn list_markers_use_tertiary_color() {
    assert!(render_markdown_line("- item").contains(&format!("{TERTIARY_STYLE}-{RESET}")));
    assert!(render_markdown_line("1. item").contains(&format!("{TERTIARY_STYLE}1.{RESET}")));
}

#[test]
fn streams_raw_table_rows_then_replaces_with_rendered_table() {
    let mut renderer = MarkdownStreamRenderer::new();
    // 1. 首行未确认：输出原文
    assert_eq!(renderer.push("| a | b |\n"), "| a | b |\n");
    // 2. 分隔行确认后：清屏并按当前列宽渲染表格
    let confirmed = renderer.push("| - | - |\n");
    assert!(confirmed.contains("\x1b[1A\r\x1b[2K"));
    assert!(confirmed.contains('┌'));

    // 3. 后续数据行：再次清屏并以最新列宽重绘整表
    let row = renderer.push("| 1 | 2 |\n");
    assert!(row.contains("\x1b[1A\r\x1b[2K"));
    assert!(row.contains('┌'));
    assert!(row.contains('1'));

    let output = renderer.push("done\n");
    assert!(output.contains("\x1b[1A\r\x1b[2K") || output.contains('┌'));
    assert!(output.contains("\x1b[1ma\x1b[0m"));
    assert!(output.contains('┌'));
    assert!(output.contains('├'));
    assert!(output.contains('└'));
    assert!(output.ends_with("done\n"));
}

#[test]
fn streaming_table_width_uses_later_rows() {
    let mut renderer = MarkdownStreamRenderer::new();
    assert_eq!(renderer.push("| 软件 | 命令 |\n"), "| 软件 | 命令 |\n");
    let confirmed = renderer.push("|---|---|\n");
    assert!(confirmed.contains('┌'));
    let first = renderer.push("| Arch | `pacman -Syu` |\n");
    assert!(first.contains("pacman -Syu"));
    let second = renderer.push("| Neovim | `sudo pacman -S neovim` |\n");
    // 更长单元格触发清屏重绘，列宽吸收后续行
    assert!(second.contains("\x1b[1A\r\x1b[2K"));
    assert!(second.contains("sudo pacman -S neovim"));
    assert!(second.contains('┌'));

    let output = renderer.flush();
    assert!(output.contains("sudo pacman -S neovim"));
    assert!(output.contains('┌'));
    assert!(output.contains('└'));
}

#[test]
fn source_preview_snapshots_open_table_with_latest_widths() {
    let mut renderer = MarkdownStreamRenderer::new_source_preview();
    assert!(renderer.push("| 软件 | 命令 |\n").is_empty());
    assert!(renderer.push("|---|---|\n").is_empty());
    assert!(renderer.push("| Arch | `pacman` |\n").is_empty());
    let preview = renderer.snapshot_open_structures();
    assert!(preview.contains('┌'));
    assert!(preview.contains("pacman"));
    assert!(renderer.push("| Neovim | `sudo pacman -S neovim` |\n").is_empty());
    let wider = renderer.snapshot_open_structures();
    assert!(wider.contains("sudo pacman -S neovim"));
    let finished = renderer.flush();
    assert!(finished.contains("sudo pacman -S neovim"));
    assert!(finished.contains('└'));
}

#[test]
fn blockquote_is_visually_distinct() {
    let mut renderer = MarkdownStreamRenderer::new();
    let output = renderer.push(">> quoted\n");
    assert!(output.contains("\x1b[32m| \x1b[0m\x1b[32m| \x1b[0m"));
    assert!(output.contains("\x1b[32mquoted\x1b[0m"));
    assert!(!output.contains("48;5;236"));
}

#[test]
fn code_block_has_label_and_readable_content() {
    let mut renderer = MarkdownStreamRenderer::new();
    let output = renderer.push("```rust\nfn main() {}\n```\n");
    assert!(output.contains("rust"));
    assert!(!output.contains("──"));
    assert!(!output.contains("-- code rust"));
    assert!(!output.contains(",-- code rust"));
    assert!(!output.contains("\x1b[2m|\x1b[0m"));
    assert!(output.contains(&format!("{CODE_KEYWORD_STYLE}fn{CODE_TOKEN_RESET}")));
    assert!(output.contains(&format!("{CODE_FUNCTION_STYLE}main{CODE_TOKEN_RESET}")));
    assert!(output.contains(&format!("{CODE_BLOCK_FRAME_STYLE}rust{RESET}")));
    assert!(!output.contains("`--"));
}

#[test]
fn code_block_streams_line_by_line() {
    let mut renderer = MarkdownStreamRenderer::new();

    // Opening ``` → header immediately
    let out = renderer.push("```rust\n");
    assert!(out.contains("rust"));
    assert!(!out.contains("──"));
    assert!(!out.contains("fn main"));

    // First code line → highlighted immediately
    let out = renderer.push("fn main() {}\n");
    assert!(out.contains("fn") || out.contains(&format!("{CODE_KEYWORD_STYLE}fn")));
    assert!(!out.contains("── rust"));
    assert!(!out.contains("────"));

    // Second code line → highlighted immediately
    let out = renderer.push("let x = 42;\n");
    assert!(out.contains("42"));

    // Closing ``` → no footer line
    let out = renderer.push("```\n");
    assert!(!out.contains("─"));
    assert!(!out.contains("fn main"));
    assert!(!out.contains("── rust"));
}

#[test]
fn code_block_suppresses_first_empty_line() {
    let mut renderer = MarkdownStreamRenderer::new();
    let output = renderer.push("```rust\nfn main() {}\n```\n\nNext\n");
    // Footer ends with \n, then the empty line is suppressed.
    // "Next" should follow immediately without a blank line.
    assert!(!output.contains("\n\nNext"));
    assert!(output.contains("\nNext"));
}

#[test]
fn code_block_suppresses_previous_empty_line() {
    let mut renderer = MarkdownStreamRenderer::new();
    let output = renderer.push("先测试：\n\n```bash\npwd\n```\n");
    assert!(!output.contains("先测试：\n\n"));
    assert!(output.contains("先测试：\n"));
    assert!(output.contains("bash"));
    assert!(!output.contains("──"));
}

#[test]
fn regular_paragraphs_suppress_single_blank_line() {
    let mut renderer = MarkdownStreamRenderer::new();
    let output = renderer.push("第一段\n\n第二段\n");
    assert!(output.contains("第一段\n第二段\n"));
    assert!(!output.contains("第一段\n\n第二段\n"));
}

#[test]
fn code_block_content_has_default_color() {
    let mut renderer = MarkdownStreamRenderer::new();
    let output = renderer.push("```\nXMODIFIERS \"@im=fcitx\"\n```\n");
    assert!(output.contains("XMODIFIERS \"@im=fcitx\"\n"));
    assert!(!output.contains("\x1b[33mXMODIFIERS"));
    assert!(!output.contains('─'));
}

#[test]
fn code_block_variables_use_primary_color() {
    let mut renderer = MarkdownStreamRenderer::new();
    let output = renderer.push("```rust\nlet msg = String::from(\"hi\");\n```\n");
    assert!(output.contains(&format!("{PRIMARY_STYLE}msg{CODE_TOKEN_RESET}")));
}

#[test]
fn code_block_keeps_lines_without_frame() {
    let mut renderer = MarkdownStreamRenderer::new();
    let output = renderer.push("```\nshort\nlonger line\n```\n");
    assert!(output.contains("short\n"));
    assert!(output.contains("longer line\n"));
    assert!(!output.contains('─'));
    assert!(!output.contains("48;5;236"));
}

#[test]
fn code_block_keeps_cjk_content_without_frame() {
    let mut renderer = MarkdownStreamRenderer::new();
    let output = renderer.push("```rust\nlet 中文 = 42;\n```\n");
    assert!(output.contains("中文"));
    assert!(!output.contains('─'));
}

#[test]
fn renders_more_inline_markdown() {
    let output = render_inline(
        "*i* ~~gone~~ [site](https://example.com) <https://example.org> ![pic](https://img)",
    );
    assert!(output.contains(&format!("{ITALIC_STYLE}i{RESET}")));
    assert!(output.contains(&format!("{STRIKE_STYLE}gone{RESET}")));
    assert!(output.contains(&format!("<{URL_STYLE}https://example.com{RESET}>")));
    assert!(output.contains(&format!(
        "\x1b[4m<{URL_STYLE}https://example.org{RESET}>{RESET}"
    )));
    assert!(output.contains(&format!(
        "{IMAGE_STYLE}[image: pic]{RESET}({URL_STYLE}https://img{RESET})"
    )));
    assert!(!output.contains("\x1b[35mimage\x1b[0m"));
}

#[test]
fn renders_inline_code_at_start_of_bullet() {
    let output = render_markdown_line("- `read_file` — 读文件内容");
    assert!(output.contains(&format!("{INLINE_CODE_STYLE}read_file\x1b[0m")));
    assert!(output.contains("— 读文件内容"));
}

#[test]
fn renders_multiple_inline_code_spans_in_bullet_with_chinese_text() {
    let output = render_markdown_line(
        "- `~/.config/Thunar/` - 里面有 `accels.scm`（快捷键绑定）和 `uca.xml`（自定义右键菜单）",
    );
    assert!(output.contains(&format!("{INLINE_CODE_STYLE}~/.config/Thunar/\x1b[0m")));
    assert!(output.contains(&format!("{INLINE_CODE_STYLE}accels.scm\x1b[0m")));
    assert!(output.contains(&format!("{INLINE_CODE_STYLE}uca.xml\x1b[0m")));
    assert!(!output.contains('`'));
}

#[test]
fn renders_inline_code_when_stream_chunks_split_backticks() {
    let mut renderer = MarkdownStreamRenderer::new();
    assert_eq!(renderer.push("- `~/.config/Thu"), "");
    let output = renderer.push("nar/` - 里面有 `accels.scm`\n");
    assert!(output.contains(&format!("{INLINE_CODE_STYLE}~/.config/Thunar/\x1b[0m")));
    assert!(output.contains(&format!("{INLINE_CODE_STYLE}accels.scm\x1b[0m")));
    assert!(!output.contains('`'));
}

#[test]
fn keeps_identifier_underscores_literal() {
    let output = render_inline("GTK_IM_MODULE and _italic_");
    assert!(output.contains("GTK_IM_MODULE"));
    assert!(output.contains(&format!("{ITALIC_STYLE}italic{RESET}")));
    assert!(!output.contains("GTK\x1b[3mIM\x1b[0mMODULE"));
    assert_eq!(render_inline("abc_def_ghi"), "abc_def_ghi");
}

#[test]
fn renders_inline_math_formulas_visibly() {
    let _guard = ASSET_STUB_LOCK.lock().unwrap();
    std::env::set_var("SAI_RENDER_ASSET_TEST_STUB", "1");
    let output = render_inline("inline $E=mc^2$ and display $$a^2+b^2=c^2$$");
    std::env::remove_var("SAI_RENDER_ASSET_TEST_STUB");
    assert!(output.contains("inline "));
    assert!(output.contains(" and display "));
    assert!(output.contains("[inline math rendering skipped]"));
    assert!(!output.contains("$E=mc^2$"));
    assert!(!output.contains("$$a^2+b^2=c^2$$"));
}

#[test]
fn source_preview_keeps_inline_math_as_source_until_finalization() {
    let _guard = ASSET_STUB_LOCK.lock().unwrap();
    std::env::set_var("SAI_RENDER_ASSET_TEST_STUB", "1");
    let mut renderer = MarkdownStreamRenderer::new_source_preview();
    let output = renderer.push("inline $E=mc^2$ and display $$a^2+b^2=c^2$$\n");
    std::env::remove_var("SAI_RENDER_ASSET_TEST_STUB");

    assert_eq!(output, "inline $E=mc^2$ and display $$a^2+b^2=c^2$$\n");
    assert!(!output.contains("[inline math rendering skipped]"));
}

#[test]
fn removes_stray_formula_prefix_at_line_start() {
    let _guard = ASSET_STUB_LOCK.lock().unwrap();
    std::env::set_var("SAI_RENDER_ASSET_TEST_STUB", "1");
    let backtick = render_inline("`$x^2$");
    let dunhao = render_inline("、$x^2$");
    let text = render_inline("文字、$x^2$");
    std::env::remove_var("SAI_RENDER_ASSET_TEST_STUB");
    assert!(!backtick.starts_with('`'));
    assert!(!dunhao.starts_with('、'));
    assert!(text.starts_with("文字、"));
}

#[test]
fn renders_multiline_math_blocks_as_assets() {
    let _guard = ASSET_STUB_LOCK.lock().unwrap();
    std::env::set_var("SAI_RENDER_ASSET_TEST_STUB", "1");
    let mut renderer = MarkdownStreamRenderer::new();
    let output = renderer.push("$$\na^2 + b^2 = c^2\n$$\n");
    std::env::remove_var("SAI_RENDER_ASSET_TEST_STUB");
    assert!(output.contains("$$\na^2 + b^2 = c^2\n$$\n"));
    assert!(output.contains("\x1b[1A\r\x1b[2K"));
    assert!(output.contains("[asset rendering skipped]"));
    assert!(!output.contains("[math]"));
}

#[test]
fn renders_mermaid_blocks_as_assets() {
    let _guard = ASSET_STUB_LOCK.lock().unwrap();
    std::env::set_var("SAI_RENDER_ASSET_TEST_STUB", "1");
    let mut renderer = MarkdownStreamRenderer::new();
    let output = renderer.push("```mermaid\ngraph TD\nA --> B\n```\n");
    std::env::remove_var("SAI_RENDER_ASSET_TEST_STUB");
    assert!(output.contains("```mermaid\ngraph TD\nA --> B\n```\n"));
    assert!(output.contains("\x1b[1A\r\x1b[2K"));
    assert!(output.contains("[asset rendering skipped]"));
    assert!(!output.contains("[mermaid]"));
    assert!(!output.contains("── mermaid"));
}

#[test]
fn renders_selected_inline_html_tags() {
    let output = render_inline("<u>under</u> H<sub>2</sub> x<sup>2</sup><br>next");
    assert!(output.contains("\x1b[4munder\x1b[0m"));
    assert!(output.contains("H\x1b[2m2\x1b[0m"));
    assert!(output.contains("x\x1b[1m2\x1b[0m"));
    assert!(output.contains("\nnext"));
}

#[test]
fn horizontal_rule_uses_terminal_width_fallback() {
    let output = render_markdown_line("---");
    assert!(output.starts_with("\x1b[2m"));
    assert!(output.ends_with("\x1b[0m"));
    assert_eq!(
        table::visible_width(&output),
        crate::render::markdown_blocks::horizontal_rule_width()
    );
}

#[test]
fn supports_table_alignment_markers() {
    let mut renderer = MarkdownStreamRenderer::new();
    let output = renderer.push("| left | mid | right |\n| :--- | :---: | ---: |\n| a | b | c |\n");
    let output = format!("{output}{}", renderer.flush());
    assert!(output.contains('┌'));
    assert!(output.contains('│'));
    assert!(!output.contains('+'));
    assert!(output.contains("\x1b[1A\r\x1b[2K"));
    assert!(output.contains("\x1b[1mleft\x1b[0m"));
}

#[test]
fn does_not_buffer_plain_lines_with_pipes_as_tables() {
    let mut renderer = MarkdownStreamRenderer::new();
    let output = renderer.push("echo hi | wc -l\nnext\n");
    assert!(output.contains("echo hi | wc -l\nnext\n"));
}

#[test]
fn table_cell_renders_images_as_compact_placeholder() {
    let output = render_table_cell("![tux](https://example.com/tux.png)");
    assert!(output.contains(&format!("{IMAGE_STYLE}[image]{RESET}")));
    assert!(!output.contains("https://example.com"));
    assert!(!output.contains('\n'));
}

#[test]
fn table_cell_content_renders_mixed_text_math_as_image() {
    let content = render_table_cell_content("公式 $E=mc^2$ 在这里");
    assert!(content.is_image);
    assert!(content
        .math_source
        .as_deref()
        .is_some_and(|s| s.starts_with("mixed:")));
    assert!(content.width >= 1);
    assert!(!content.lines.is_empty());
}

#[test]
fn table_cell_content_renders_pure_math_as_image() {
    let content = render_table_cell_content("$E=mc^2$");
    assert!(content.is_image);
    assert!(content
        .math_source
        .as_deref()
        .is_some_and(|s| s.starts_with("pure:")));
}

#[test]
fn table_cell_renders_display_math_as_halfblock_image() {
    let output = render_table_cell("$$a^2+b^2$$");
    assert!(output.contains('▀') || output.contains('▄'));
    assert!(!output.contains('\n'));
}

#[test]
fn table_cell_renders_links_as_label_only() {
    let output = render_table_cell("[点我去 ArchWiki](https://wiki.archlinux.org)");
    assert!(output.contains(&format!("{LINK_LABEL_STYLE}点我去 ArchWiki{RESET}")));
    assert!(!output.contains("https://wiki.archlinux.org"));
    assert!(!output.contains('\n'));
}

#[test]
fn table_cell_preserves_bold_italic_code() {
    let output = render_table_cell("**加粗** _斜体_ `代码`");
    assert!(output.contains(&format!("{BOLD_STYLE}加粗{RESET}")));
    assert!(output.contains(&format!("{ITALIC_STYLE}斜体{RESET}")));
    assert!(output.contains(&format!("{INLINE_CODE_STYLE}代码{RESET}")));
    assert!(!output.contains('\n'));
}

#[test]
fn table_cell_collapses_list_items() {
    let output = render_table_cell("- 第一项\n- 第二项\n- 第三项");
    assert!(output.contains("第一项"));
    assert!(output.contains("第二项"));
    assert!(output.contains("第三项"));
    assert!(!output.contains('\n'));
    assert!(output.contains("·"));
}

#[test]
fn table_cell_collapses_blockquotes() {
    let output = render_table_cell("> 第一层\n> > 第二层");
    assert!(output.contains("第一层"));
    assert!(output.contains("第二层"));
    assert!(!output.contains('\n'));
    assert!(!output.contains("> "));
}

#[test]
fn table_cell_handles_br_tags() {
    let output = render_table_cell("- 第一项<br>- 第二项<br>- 第三项");
    assert!(output.contains("第一项"));
    assert!(output.contains("第二项"));
    assert!(!output.contains("<br>"));
    assert!(!output.contains('\n'));
}

#[test]
fn table_with_mixed_inline_markdown_stays_aligned() {
    let output = table::render_table(
        &[
            "| 类型 | 示例 |".to_string(),
            "|---|---|".to_string(),
            "| 加粗 | **粗体文字** |".to_string(),
            "| 图片 | ![tux](https://example.com/tux.png) |".to_string(),
            "| 公式 | $E=mc^2$ |".to_string(),
            "| 链接 | [ArchWiki](https://wiki.archlinux.org) |".to_string(),
        ],
        render_table_cell_content,
    );
    for line in output.lines() {
        let width = table::visible_width(line);
        let next = output
            .lines()
            .map(|l| table::visible_width(l))
            .max()
            .unwrap_or(0);
        assert!(
            width <= next,
            "line wider than max: {line} (width={width}, max={next})"
        );
    }
    assert!(!output.contains("https://"));
    let border_count = output.matches('├').count();
    assert_eq!(border_count, 4, "expected 4 middle borders for 5 rows");
}
