//! 终端身份与单元格像素尺寸解析，屏蔽平台差异并便于无副作用测试。

const MIN_CELL_PIXEL_SIZE: usize = 1;
const MAX_CELL_PIXEL_SIZE: usize = 256;

/// 【终端图片】【复用器检测】判断当前是否运行在默认不透传图形协议的终端复用器中。
///
/// 参数:
/// - `tmux`: `TMUX` 环境变量是否存在
/// - `tmux_pane`: `TMUX_PANE` 环境变量是否存在
/// - `zellij`: `ZELLIJ` 或 `ZELLIJ_SESSION_NAME` 环境变量是否存在
/// - `screen`: `STY` 环境变量是否存在
///
/// 返回:
/// - 处于复用器内时返回 true
pub(super) const fn is_multiplexer_env(
    tmux: bool,
    tmux_pane: bool,
    zellij: bool,
    screen: bool,
) -> bool {
    tmux || tmux_pane || zellij || screen
}

/// 【终端图片】【环境识别】结合环境变量判断当前终端是否原生支持 Kitty 图形协议。
///
/// Windows ConPTY 会吞掉所有 APC 输入应答，导致主动查询在 ELYTerminal 等终端下必然超时；
/// 因此必须通过 `TERM_PROGRAM` 与 `TERM` 识别已知 Kitty 终端，避免被误判降级。
///
/// 参数:
/// - `kitty_window_id`: 是否存在 `KITTY_WINDOW_ID`
/// - `multiplexer`: 是否处于 tmux/zellij/screen 复用器内
/// - `term`: `TERM` 环境变量值
/// - `term_program`: `TERM_PROGRAM` 环境变量值
///
/// 返回:
/// - 已知支持 Kitty 图形协议时返回 true
pub(super) fn known_kitty_graphics_from_env(
    kitty_window_id: bool,
    multiplexer: bool,
    term: Option<&str>,
    term_program: Option<&str>,
) -> bool {
    // 1. 【终端图片】【Kitty 窗口】显式窗口标识优先于复用器保守策略，保持既有兼容
    if kitty_window_id {
        return true;
    }
    // 2. 【终端图片】【复用器保护】tmux/zellij/screen 默认不透传图形 APC，避免输出乱码
    if multiplexer {
        return false;
    }
    // 3. 【终端图片】【TERM 匹配】识别 Kitty、Ghostty、Foot 等专用 terminfo
    if let Some(term) = term.map(str::trim).filter(|value| !value.is_empty()) {
        let lower = term.to_ascii_lowercase();
        if lower == "kitty"
            || lower.contains("xterm-kitty")
            || lower.starts_with("xterm-ghostty")
            || lower.starts_with("foot")
        {
            return true;
        }
    }
    // 4. 【终端图片】【宿主标识】大小写不敏感匹配已知 Kitty 图形终端（含 Windows/WSL ELYTerminal）
    let Some(program) = term_program
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    matches!(
        program.to_ascii_lowercase().as_str(),
        "elyterminal" | "ghostty" | "kitty" | "foot" | "konsole" | "rio"
    )
}

/// 【终端图片】【当前环境】读取进程环境变量，判断是否为已知 Kitty 图形终端。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 已知支持 Kitty 图形协议时返回 true
pub(super) fn known_kitty_graphics_env() -> bool {
    let multiplexer = is_multiplexer_env(
        std::env::var_os("TMUX").is_some(),
        std::env::var_os("TMUX_PANE").is_some(),
        std::env::var_os("ZELLIJ").is_some() || std::env::var_os("ZELLIJ_SESSION_NAME").is_some(),
        std::env::var_os("STY").is_some(),
    );
    let term = std::env::var("TERM").ok();
    let term_program = std::env::var("TERM_PROGRAM").ok();
    known_kitty_graphics_from_env(
        std::env::var_os("KITTY_WINDOW_ID").is_some(),
        multiplexer,
        term.as_deref(),
        term_program.as_deref(),
    )
}

/// 【终端图片】【单元格像素】解析 CSI 16 t 的响应正文 `6;<height>;<width>`。
///
/// 参数:
/// - `body`: 去掉 `\x1b[` 前缀与 `t` 终结符后的参数正文
///
/// 返回:
/// - 合法范围内返回 `Some((cell_width_px, cell_height_px))`
pub(super) fn parse_cell_pixel_size(body: &str) -> Option<(usize, usize)> {
    let mut parts = body.split(';');
    if parts.next()? != "6" {
        return None;
    }
    let height = parts.next()?.parse::<usize>().ok()?;
    let width = parts.next()?.parse::<usize>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    if !(MIN_CELL_PIXEL_SIZE..=MAX_CELL_PIXEL_SIZE).contains(&width)
        || !(MIN_CELL_PIXEL_SIZE..=MAX_CELL_PIXEL_SIZE).contains(&height)
    {
        return None;
    }
    Some((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端图片测试】【ELYTerminal】通用 TERM 下凭借 TERM_PROGRAM 识别 Kitty 支持。
    #[test]
    fn recognizes_elyterminal_and_known_kitty_hosts_case_insensitively() {
        for program in [
            "elyterminal",
            "ELYTerminal",
            "ghostty",
            "Ghostty",
            "kitty",
            "foot",
            "Konsole",
            "rio",
        ] {
            assert!(
                known_kitty_graphics_from_env(false, false, Some("xterm-256color"), Some(program)),
                "expected Kitty support for TERM_PROGRAM={program}"
            );
        }
        assert!(known_kitty_graphics_from_env(
            false,
            false,
            Some("xterm-kitty"),
            None
        ));
        assert!(known_kitty_graphics_from_env(
            false,
            false,
            Some("xterm-ghostty"),
            None
        ));
        assert!(known_kitty_graphics_from_env(
            false,
            false,
            Some("foot-extra"),
            None
        ));
        assert!(!known_kitty_graphics_from_env(
            false,
            false,
            Some("xterm-256color"),
            Some("Windows_Terminal")
        ));
        assert!(!known_kitty_graphics_from_env(
            false,
            false,
            Some("xterm-256color"),
            Some("WezTerm")
        ));
    }

    /// 【终端图片测试】【复用器保护】复用器内阻止按宿主推断，显式 KITTY_WINDOW_ID 保留兼容。
    #[test]
    fn respects_multiplexer_guard_and_kitty_window_id_priority() {
        assert!(!known_kitty_graphics_from_env(
            false,
            true,
            Some("xterm-256color"),
            Some("elyterminal")
        ));
        assert!(known_kitty_graphics_from_env(
            true,
            true,
            Some("screen-256color"),
            None
        ));
    }

    /// 【终端图片测试】【单元格像素】按 CSI 16 t 规范以高度在前、宽度在后解析并校验边界。
    #[test]
    fn parses_csi_16t_cell_pixel_size_with_height_then_width() {
        assert_eq!(parse_cell_pixel_size("6;16;8"), Some((8, 16)));
        assert_eq!(parse_cell_pixel_size("6;24;12"), Some((12, 24)));
        assert_eq!(parse_cell_pixel_size("6;0;8"), None);
        assert_eq!(parse_cell_pixel_size("6;16;0"), None);
        assert_eq!(parse_cell_pixel_size("6;300;12"), None);
        assert_eq!(parse_cell_pixel_size("4;16;8"), None);
        assert_eq!(parse_cell_pixel_size("6;16;8;1"), None);
    }
}
