mod commands;
mod math;
mod mermaid;
mod table_math;

#[cfg(test)]
mod tests;

use crate::render::terminal_text as t;
use crate::render::style::{ASSET_ERROR_STYLE, RESET};

pub(crate) use table_math::{
    decode_source as decode_table_math_source, render_cell as render_inline_math_table_cell,
    render_inline_halfblock as render_inline_math_halfblock,
};

#[derive(Clone, Copy)]
enum AssetKind {
    Mermaid,
    Math,
}

#[derive(Clone, Copy)]
pub(super) enum MathRenderMode {
    Block,
    Inline,
}

impl AssetKind {
    /// 返回资产类型展示名称。
    ///
    /// 返回:
    /// - 资产类型名称
    fn label(self) -> &'static str {
        match self {
            Self::Mermaid => "mermaid",
            Self::Math => "math",
        }
    }
}

/// 判断代码块语言是否需要渲染为图片资产。
///
/// 参数:
/// - `lang`: Markdown 代码块语言
///
/// 返回:
/// - 是否为 Mermaid 或数学资产
pub(crate) fn is_asset_language(lang: &str) -> bool {
    asset_kind_from_lang(lang).is_some()
}

/// 渲染 Markdown 图片资产代码块。
///
/// 参数:
/// - `lang`: Markdown 代码块语言
/// - `lines`: 代码块内容
///
/// 返回:
/// - 终端图片协议文本或错误提示
pub(crate) fn render_asset_block(lang: &str, lines: &[String]) -> String {
    let Some(kind) = asset_kind_from_lang(lang) else {
        return render_error("asset", t("unsupported asset language", "不支持的资源语言"));
    };
    render_asset(kind, &lines.join("\n"))
}

/// 渲染块级数学公式。
///
/// 参数:
/// - `lines`: 数学公式内容行
///
/// 返回:
/// - 终端图片协议文本或错误提示
pub(crate) fn render_math_block(lines: &[String]) -> String {
    math::render_source(&lines.join("\n"), MathRenderMode::Block)
}

/// 渲染行内数学公式。
///
/// 参数:
/// - `source`: 数学公式源码
///
/// 返回:
/// - 终端图片协议文本或错误提示
pub(crate) fn render_inline_math(source: &str) -> String {
    math::render_source(source, MathRenderMode::Inline)
}

/// 解析 Markdown 资产语言。
///
/// 参数:
/// - `lang`: Markdown 代码块语言
///
/// 返回:
/// - 资产类型
fn asset_kind_from_lang(lang: &str) -> Option<AssetKind> {
    match lang.trim().to_ascii_lowercase().as_str() {
        "mermaid" | "mmd" => Some(AssetKind::Mermaid),
        "math" | "latex" | "tex" => Some(AssetKind::Math),
        _ => None,
    }
}

/// 渲染单个图片资产。
///
/// 参数:
/// - `kind`: 资产类型
/// - `source`: 原始内容
///
/// 返回:
/// - 终端图片协议文本或错误提示
fn render_asset(kind: AssetKind, source: &str) -> String {
    if source.trim().is_empty() {
        return render_error(kind.label(), t("content is empty", "内容为空"));
    }
    if matches!(kind, AssetKind::Math) {
        return math::render_source(source, MathRenderMode::Block);
    }
    if test_stub_enabled() {
        return render_success("[asset rendering skipped]\n".to_string());
    }
    match mermaid::render_terminal(source) {
        Ok(rendered) => render_success(rendered),
        Err(error) => render_error(kind.label(), &error.to_string()),
    }
}

/// 返回成功的图片渲染文本。
///
/// 参数:
/// - `rendered`: 图片协议文本
///
/// 返回:
/// - 原始图片协议文本
pub(super) fn render_success(rendered: String) -> String {
    rendered
}

/// 渲染资产错误提示。
///
/// 参数:
/// - `label`: 资产类型标签
/// - `message`: 错误信息
///
/// 返回:
/// - 带样式的错误提示
pub(super) fn render_error(label: &str, message: &str) -> String {
    format!("{ASSET_ERROR_STYLE}[{label} render failed: {message}]{RESET}\n")
}

/// 判断测试替身是否开启。
///
/// 返回:
/// - 是否跳过实际图片生成
pub(super) fn test_stub_enabled() -> bool {
    cfg!(test) && std::env::var_os("SAI_RENDER_ASSET_TEST_STUB").is_some()
}
