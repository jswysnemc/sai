use std::io::IsTerminal;
use std::sync::OnceLock;
use std::time::Duration;

type Rgb = (u8, u8, u8);

static DEFAULT_COLORS: OnceLock<Option<(Rgb, Rgb)>> = OnceLock::new();
static TRUE_COLOR: OnceLock<bool> = OnceLock::new();

/// 【终端】【颜色缓存】状态动效使用的默认前景、背景和真彩色能力。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TerminalPalette {
    pub(crate) foreground: Rgb,
    pub(crate) background: Rgb,
    pub(crate) true_color: bool,
}

/// 【终端】【颜色探测】在输入循环启动前查询一次终端颜色，避免与键盘事件争用输入。
/// 参数：无；返回：无，查询失败时由读取端使用默认颜色。
pub(crate) fn initialize_terminal_palette() {
    DEFAULT_COLORS.get_or_init(|| {
        // 1. 【终端】【颜色探测】重定向输出或非交互输入不发送终端查询
        if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
            return None;
        }
        // 2. 【终端】【颜色探测】限制启动等待时间，并由查询库恢复原有终端模式
        let mut options = terminal_colorsaurus::QueryOptions::default();
        options.timeout = Duration::from_millis(200);
        terminal_colorsaurus::color_palette(options)
            .ok()
            .map(|colors| {
                (
                    colors.foreground.scale_to_8bit(),
                    colors.background.scale_to_8bit(),
                )
            })
    });
}

/// 【终端】【颜色缓存】读取启动时缓存的颜色；逐帧渲染不会触发终端查询。
/// 参数：无；返回：终端颜色与缓存的真彩色能力。
pub(crate) fn terminal_palette() -> TerminalPalette {
    let colors = DEFAULT_COLORS.get().copied().flatten();
    let true_color = *TRUE_COLOR.get_or_init(|| {
        let detected = supports_color::on_cached(supports_color::Stream::Stdout)
            .is_some_and(|level| level.has_16m);
        resolve_true_color(
            detected,
            std::env::var_os("WT_SESSION").is_some(),
            std::env::var_os("FORCE_COLOR").is_some(),
        )
    });
    resolve_palette(colors, true_color)
}

/// 【终端】【颜色缓存】补齐与 Codex 相同的缺省前景和背景颜色。
/// 参数：`colors` 为可选探测结果，`true_color` 为颜色能力；返回：完整调色板。
fn resolve_palette(colors: Option<(Rgb, Rgb)>, true_color: bool) -> TerminalPalette {
    let (foreground, background) = colors.unwrap_or(((128, 128, 128), (255, 255, 255)));
    TerminalPalette {
        foreground,
        background,
        true_color,
    }
}

/// 【终端】【颜色能力】补充 Windows Terminal 的真彩色识别，并尊重显式环境覆盖。
/// 参数：`detected` 为库探测结果，后两项表示终端标记和覆盖变量；返回：是否使用真彩色。
fn resolve_true_color(detected: bool, windows_terminal: bool, force_override: bool) -> bool {
    detected || (windows_terminal && !force_override)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【颜色测试】探测失败时使用 Codex 缺省色，成功时保留真实主题颜色。
    /// 参数：无；返回：无。
    #[test]
    fn palette_uses_detected_colors_or_codex_defaults() {
        let fallback = resolve_palette(None, false);
        assert_eq!(fallback.foreground, (128, 128, 128));
        assert_eq!(fallback.background, (255, 255, 255));
        assert!(!fallback.true_color);
        for colors in [
            ((230, 230, 230), (24, 24, 24)),
            ((30, 30, 30), (250, 250, 250)),
        ] {
            let palette = resolve_palette(Some(colors), true);
            assert_eq!((palette.foreground, palette.background), colors);
            assert!(palette.true_color);
        }
    }

    /// 【终端】【颜色测试】Windows Terminal 仅在没有显式覆盖时补充真彩色能力。
    /// 参数：无；返回：无。
    #[test]
    fn windows_terminal_respects_explicit_color_override() {
        assert!(resolve_true_color(false, true, false));
        assert!(!resolve_true_color(false, true, true));
        assert!(!resolve_true_color(false, false, false));
        assert!(resolve_true_color(true, true, true));
    }
}
