//! 上下文数量与物理占用率的紧凑格式。

/// 【终端指标】【数量缩写】参数 value 为 token 数；返回使用 k/M 单位的紧凑数值。
pub(super) fn tokens(value: usize) -> String {
    if value >= 1_000_000 {
        format!("{}M", decimal(value as f64 / 1_000_000.0))
    } else if value >= 1_000 {
        format!("{}k", decimal(value as f64 / 1_000.0))
    } else {
        value.to_string()
    }
}

/// 【终端指标】【小数精度】参数 value 为缩放数；返回最多一位小数，整数不保留尾零。
fn decimal(value: f64) -> String {
    format!("{value:.1}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

/// 【终端指标】【上下文读数】参数 context 为已用量与窗口；返回数值和实际百分比，零窗口视为未知。
pub(super) fn context(context: Option<(usize, usize)>) -> String {
    context.filter(|(_, window)| *window > 0).map_or_else(
        || "-- / -- · --%".into(),
        |(used, window)| {
            format!(
                "{} / {} · {:.1}%",
                tokens(used),
                tokens(window),
                (used as f64 / window as f64 * 1000.0).round() / 10.0
            )
        },
    )
}
