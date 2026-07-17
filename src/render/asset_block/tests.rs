use super::*;
use std::fs;
use std::path::Path;

#[test]
fn detects_asset_languages() {
    assert!(is_asset_language("mermaid"));
    assert!(is_asset_language("math"));
    assert!(is_asset_language("latex"));
    assert!(!is_asset_language("rust"));
}

#[test]
fn math_svg_escapes_formula_text() {
    let svg = math::build_fallback_svg(r#"a < b && c > "d""#, MathRenderMode::Block);

    assert!(svg.contains("&lt;"));
    assert!(svg.contains("&gt;"));
    assert!(svg.contains("&quot;"));
}

#[test]
fn ratex_renders_common_formulas_to_png() {
    let temp_dir = tempfile::tempdir().unwrap();
    let formulas = [
        r"\int_a^b f(x)\,dx = F(b) - F(a)",
        r"\lim_{x \to 0} \frac{\sin x}{x} = 1",
        r"P(A|B) = \frac{P(B|A)\,P(A)}{P(B)}",
    ];

    for formula in formulas {
        let output = math::try_render_ratex(formula, &temp_dir, MathRenderMode::Block)
            .unwrap()
            .expect("RaTeX should render this formula");
        assert!(fs::metadata(output).unwrap().len() > 0);
    }
}

#[test]
fn ratex_renders_inline_formula_to_png() {
    let temp_dir = tempfile::tempdir().unwrap();
    let output = math::try_render_ratex(
        r"\lim_{x \to 0} \frac{\sin x}{x} = 1",
        &temp_dir,
        MathRenderMode::Inline,
    )
    .unwrap()
    .expect("RaTeX should render inline formula");

    assert!(fs::metadata(output).unwrap().len() > 0);
}

#[test]
fn mermaid_renders_to_png_without_external_cli() {
    let temp_dir = tempfile::tempdir().unwrap();
    let output = mermaid::render_image("graph TD\nA[Start] --> B[End]", &temp_dir).unwrap();

    assert!(fs::metadata(output).unwrap().len() > 0);
}

#[test]
fn mermaid_png_preserves_transparent_canvas() {
    let temp_dir = tempfile::tempdir().unwrap();
    let output = mermaid::render_image("graph TD\nA[Start] --> B[End]", &temp_dir).unwrap();

    assert!(png_has_transparent_pixel(&output));
}

/// 判断 PNG 是否包含透明像素。
///
/// 参数:
/// - `path`: PNG 文件路径
///
/// 返回:
/// - 是否存在透明像素
fn png_has_transparent_pixel(path: &Path) -> bool {
    let file = fs::File::open(path).unwrap();
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder.read_info().unwrap();
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).unwrap();
    let bytes = &buffer[..info.buffer_size()];
    match (info.color_type, info.bit_depth) {
        (png::ColorType::Rgba, png::BitDepth::Eight) => {
            bytes.chunks_exact(4).any(|chunk| chunk[3] == 0)
        }
        (png::ColorType::Rgba, png::BitDepth::Sixteen) => {
            bytes.chunks_exact(8).any(|chunk| chunk[6] == 0)
        }
        (png::ColorType::GrayscaleAlpha, png::BitDepth::Eight) => {
            bytes.chunks_exact(2).any(|chunk| chunk[1] == 0)
        }
        (png::ColorType::GrayscaleAlpha, png::BitDepth::Sixteen) => {
            bytes.chunks_exact(4).any(|chunk| chunk[2] == 0)
        }
        _ => false,
    }
}
