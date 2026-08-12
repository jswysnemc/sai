use crate::render::style::TOOL_BULLET;

/// 工具卡状态语义：决定行首圆点的颜色层级。
///
/// 统一视觉规则：进行中弱化（dim）、成功绿、失败红、跳过等中性状态弱化，
/// 让状态色集中在圆点与行尾徽标两个点缀位，标题正文保持默认色。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ToolHealth {
    /// 进行中 / 参数流式接收中
    Pending,
    /// 成功定稿
    Ok,
    /// 失败定稿
    Err,
    /// 中性（跳过、无结果语义）
    Neutral,
}

impl ToolHealth {
    /// 由状态键推导状态语义。
    ///
    /// 参数:
    /// - `status`: 工具状态键（ok/err/run/arg/skip 或自定义徽标）
    ///
    /// 返回:
    /// - 对应状态语义；未知徽标按进行中处理（编辑类 `+N -M` 的默认阶段）
    pub(crate) fn from_status(status: &str) -> Self {
        match status {
            "ok" => Self::Ok,
            "err" => Self::Err,
            "skip" => Self::Neutral,
            _ => Self::Pending,
        }
    }
}

/// 渲染统一的工具行首圆点。
///
/// 参数:
/// - `health`: 工具状态语义
///
/// 返回:
/// - 带 ANSI 颜色的行首圆点
pub(crate) fn tool_bullet(health: ToolHealth) -> String {
    match health {
        ToolHealth::Ok => format!("\x1b[1m\x1b[32m{TOOL_BULLET}\x1b[0m"),
        ToolHealth::Err => format!("\x1b[1m\x1b[31m{TOOL_BULLET}\x1b[0m"),
        ToolHealth::Pending | ToolHealth::Neutral => format!("\x1b[2m{TOOL_BULLET}\x1b[0m"),
    }
}

/// 为工具状态徽标添加终端颜色。
///
/// 参数:
/// - `status`: 工具状态，常见取值为 arg、run、ok、err 或 skip
///
/// 返回:
/// - 带 ANSI 颜色的状态文本，未知状态（如编辑类 `+N -M`）原样透传
pub(crate) fn color_status(status: &str) -> String {
    match status {
        "ok" => "\x1b[32mok\x1b[0m".to_string(),
        "err" => "\x1b[31merr\x1b[0m".to_string(),
        "run" => "\x1b[33mrun\x1b[0m".to_string(),
        // 持久子智能体待命：存活但不执行，用蓝色与运行中区分
        "idle" => "\x1b[38;5;110midle\x1b[0m".to_string(),
        "arg" => "\x1b[2m\x1b[36m…\x1b[0m".to_string(),
        "skip" => "\x1b[2mskip\x1b[0m".to_string(),
        value => value.to_string(),
    }
}

/// 为运行中状态标签添加终端颜色。
///
/// 参数:
/// - `label`: 本地化后的运行中标签
///
/// 返回:
/// - 带 ANSI 颜色的运行中标签
pub(crate) fn color_running(label: &str) -> String {
    format!("\x1b[33m{label}\x1b[0m")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_status_styles_known_values() {
        assert_eq!(color_status("ok"), "\x1b[32mok\x1b[0m");
        assert_eq!(color_status("err"), "\x1b[31merr\x1b[0m");
        assert_eq!(color_status("run"), "\x1b[33mrun\x1b[0m");
        assert_eq!(color_status("arg"), "\x1b[2m\x1b[36m…\x1b[0m");
        assert_eq!(color_status("skip"), "\x1b[2mskip\x1b[0m");
        assert_eq!(color_status("custom"), "custom");
    }

    #[test]
    fn color_running_styles_localized_label() {
        assert_eq!(color_running("running"), "\x1b[33mrunning\x1b[0m");
        assert_eq!(color_running("运行中"), "\x1b[33m运行中\x1b[0m");
    }

    /// 圆点颜色随状态语义变化：成功绿、失败红、进行中弱化。
    #[test]
    fn tool_bullet_reflects_health() {
        assert_eq!(tool_bullet(ToolHealth::Ok), "\x1b[1m\x1b[32m•\x1b[0m");
        assert_eq!(tool_bullet(ToolHealth::Err), "\x1b[1m\x1b[31m•\x1b[0m");
        assert_eq!(tool_bullet(ToolHealth::Pending), "\x1b[2m•\x1b[0m");
        assert_eq!(tool_bullet(ToolHealth::Neutral), "\x1b[2m•\x1b[0m");
    }

    /// 状态键正确映射到状态语义。
    #[test]
    fn health_maps_status_keys() {
        assert_eq!(ToolHealth::from_status("ok"), ToolHealth::Ok);
        assert_eq!(ToolHealth::from_status("err"), ToolHealth::Err);
        assert_eq!(ToolHealth::from_status("run"), ToolHealth::Pending);
        assert_eq!(ToolHealth::from_status("arg"), ToolHealth::Pending);
        assert_eq!(ToolHealth::from_status("skip"), ToolHealth::Neutral);
        assert_eq!(ToolHealth::from_status(""), ToolHealth::Pending);
    }
}
