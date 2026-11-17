//! 轮次总览的标签集：默认用 Nerd Font 图标，未安装字体时回退为字符箭头与文字。

/// 总览各段使用的标签。
#[derive(Clone, Copy, Debug)]
pub(crate) struct SummaryLabels {
    /// 本轮总耗时前缀
    pub worked: &'static str,
    /// 首字延迟前缀
    pub first_word: &'static str,
    /// 输入 token
    pub input: &'static str,
    /// 输出 token
    pub output: &'static str,
    /// 输入中命中缓存的比例
    pub cached: &'static str,
    /// 生成速率
    pub speed: &'static str,
    /// token 数与速率的单位
    pub tokens: &'static str,
}

/// Nerd Font 图标（BMP 私有区 Font Awesome 码点，兼容 Nerd Fonts v2/v3）。
pub(crate) const ICON_LABELS: SummaryLabels = SummaryLabels {
    worked: "Worked for",
    first_word: "TTFT",
    // nf-fa-arrow_up / nf-fa-arrow_down
    input: "\u{f062}",
    output: "\u{f063}",
    // nf-fa-refresh（BMP U+F021）：环形刷新箭头，表达缓存复用
    cached: "\u{f021}",
    // nf-fa-tachometer（BMP U+F0E4）：速率仪表盘，兼容 v2 与 v3
    speed: "\u{f0e4}",
    tokens: "toks",
};

/// 未安装 Nerd Font 时的字符标签。
pub(crate) const TEXT_LABELS: SummaryLabels = SummaryLabels {
    worked: "Worked for",
    first_word: "TTFT",
    input: "↑",
    output: "↓",
    cached: "cached",
    speed: "speed",
    tokens: "toks",
};

/// 【会话总览】【标签选择】复用全局 `nerd_font_enabled()` 按环境选择图标或纯字符标签集。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 当前终端使用的标签集
pub(crate) fn summary_labels() -> SummaryLabels {
    if crate::render::nerd_font::nerd_font_enabled() {
        ICON_LABELS
    } else {
        TEXT_LABELS
    }
}
