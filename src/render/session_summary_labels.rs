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

/// Nerd Font 图标（Font Awesome / Material 区段，主流 Nerd Font 均包含）。
pub(crate) const ICON_LABELS: SummaryLabels = SummaryLabels {
    worked: "Worked for",
    first_word: "TTFT",
    // nf-fa-arrow_up / nf-fa-arrow_down
    input: "\u{f062}",
    output: "\u{f063}",
    // nf-md-cached：环形箭头，语义即「缓存复用」
    cached: "\u{f00e8}",
    // nf-md-speedometer
    speed: "\u{f04c5}",
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

/// 按环境选择标签集。
///
/// `SAI_NERD_FONT` 为 `0` / `false` / `off` 时使用字符标签，其余情况使用图标。
///
/// 返回:
/// - 当前终端使用的标签集
pub(crate) fn summary_labels() -> SummaryLabels {
    match std::env::var("SAI_NERD_FONT") {
        Ok(value)
            if matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "off"
            ) =>
        {
            TEXT_LABELS
        }
        _ => ICON_LABELS,
    }
}
