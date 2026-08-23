mod commands;
mod math;
mod mermaid;
mod svg;
mod table_math;

#[cfg(test)]
mod tests;

use crate::render::style::{ASSET_ERROR_STYLE, RESET};
use crate::render::terminal_text as t;

pub(crate) use svg::{contains_svg_close, looks_like_svg_start};
pub(crate) use table_math::{
    decode_source as decode_table_math_source, render_cell as render_inline_math_table_cell,
    render_inline_halfblock as render_inline_math_halfblock,
};

#[derive(Clone, Copy)]
enum AssetKind {
    Mermaid,
    Math,
    Svg,
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
            Self::Svg => "svg",
        }
    }
}

/// 判断代码块语言是否需要渲染为图片资产。
///
/// 参数:
/// - `lang`: Markdown 代码块语言
///
/// 返回:
/// - 是否为 Mermaid、数学或 SVG 资产
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
    let source = lines.join("\n");
    render_cached(kind.label(), &source, || render_asset(kind, &source))
}

/// 渲染块级数学公式。
///
/// 参数:
/// - `lines`: 数学公式内容行
///
/// 返回:
/// - 终端图片协议文本或错误提示
pub(crate) fn render_math_block(lines: &[String]) -> String {
    let source = lines.join("\n");
    render_cached("math-block", &source, || {
        math::render_source(&source, MathRenderMode::Block)
    })
}

/// 渲染行内数学公式。
///
/// 参数:
/// - `source`: 数学公式源码
///
/// 返回:
/// - 终端图片协议文本或错误提示
pub(crate) fn render_inline_math(source: &str) -> String {
    render_cached("math-inline", source, || {
        math::render_source(source, MathRenderMode::Inline)
    })
}

/// 资产渲染缓存上限（超出时整体清空，公式/图表数量通常远低于该值）。
const ASSET_CACHE_LIMIT: usize = 128;

/// 以（类型 + 源码 + 终端宽度）为键缓存渲染产物。
///
/// 两个目的：
/// 1. live 预览每次重绘都会重放完整 Markdown 源，无缓存时每帧都会
///    重新生成 PNG，流式期间卡顿明显；
/// 2. 渲染产物内含 Kitty 图像/放置 ID，字节一致时重打走 Kitty 的
///    「替换」语义，位置更新而不叠影。
///
/// 参数:
/// - `kind`: 资产类型标签
/// - `source`: 原始内容
/// - `render`: 未命中时的实际渲染函数
///
/// 返回:
/// - 渲染产物（命中时为缓存值）
fn render_cached(kind: &str, source: &str, render: impl FnOnce() -> String) -> String {
    use std::collections::HashMap;
    use std::hash::{Hash, Hasher};
    use std::sync::{LazyLock, Mutex};

    static CACHE: LazyLock<Mutex<HashMap<u64, String>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    // 测试替身输出与真实渲染互斥，不入缓存避免测试间串扰
    if test_stub_enabled() {
        return render();
    }

    let mut hasher = std::hash::DefaultHasher::new();
    kind.hash(&mut hasher);
    source.hash(&mut hasher);
    // 终端宽度影响块级图的占位行列，resize 后需要重新渲染
    crossterm::terminal::size()
        .unwrap_or((80, 24))
        .hash(&mut hasher);
    let key = hasher.finish();

    if let Ok(cache) = CACHE.lock() {
        if let Some(cached) = cache.get(&key) {
            return cached.clone();
        }
    }
    let rendered = render();
    if let Ok(mut cache) = CACHE.lock() {
        if cache.len() >= ASSET_CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(key, rendered.clone());
    }
    rendered
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
        "svg" => Some(AssetKind::Svg),
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
    let rendered = match kind {
        AssetKind::Mermaid => mermaid::render_terminal(source),
        AssetKind::Svg => svg::render_terminal(source),
        AssetKind::Math => unreachable!("math already returned"),
    };
    match rendered {
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
