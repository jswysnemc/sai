//! 全屏可点击内容的悬停高亮，保色相按比例提亮代码着色，未着色文本使用主题强调色。

use super::sidebar_palette::{fg, Rgb, SidebarPalette};
use crate::render::terminal_contrast::{blend, readable_color_on, MIN_TEXT_CONTRAST};
use crate::render::terminal_palette::terminal_palette;

const RESET: &str = "\x1b[0m";
const DARK_LIFT_AMOUNT: f64 = 0.32;
const LIGHT_DEEPEN_AMOUNT: f64 = 0.30;

/// 【终端交互】【悬停颜色】参数：无；返回：主题前景并取消暗淡属性
pub(super) fn hover_default_fg() -> String {
    format!("\x1b[22m{}", fg(SidebarPalette::current().accent))
}

/// 【全屏视图】【悬停高亮】保留各语法 token 的色相并按终端明暗比例提亮或加深，未着色处使用主题强调色。
///
/// 不铺底、不加粗。保留差异背景、下划线颜色与图片协议，同时取消暗淡属性让文字清晰可读。
///
/// 参数:
/// - `line`: 已适配正文宽度的 ANSI 行
///
/// 返回:
/// - 带保色相悬停提亮的行
pub(in crate::cli::repl_runtime) fn tint_line(line: &str) -> String {
    let background = terminal_palette().background;
    tint_line_on(line, background)
}

/// 【全屏视图】【指定底色悬停】按显式终端背景亮度对文本执行保色相悬停处理。
///
/// 参数:
/// - `line`: 原始 ANSI 文本行
/// - `background`: 终端背景 RGB
///
/// 返回:
/// - 处理后的 ANSI 文本行
fn tint_line_on(line: &str, background: Rgb) -> String {
    let mut output = String::with_capacity(line.len() + 64);
    output.push_str(&hover_default_fg());
    let mut index = 0usize;
    while index < line.len() {
        if line.as_bytes()[index] == 0x1b {
            let end = crate::render::terminal_image::escape_sequence_end(line, index)
                .max(index + 1)
                .min(line.len());
            output.push_str(&rewrite_sequence(&line[index..end], background));
            index = end;
            continue;
        }
        let Some(ch) = line[index..].chars().next() else {
            break;
        };
        output.push(ch);
        index += ch.len_utf8();
    }
    output.push_str(RESET);
    output
}

/// 【全屏视图】【样式改写】将 SGR 前景色做保色相提亮；复位后补回默认悬停强调色。
///
/// 参数:
/// - `sequence`: 单个转义序列
/// - `background`: 当前终端背景 RGB
///
/// 返回:
/// - 改写后的序列
fn rewrite_sequence(sequence: &str, background: Rgb) -> String {
    let Some(params) = sequence
        .strip_prefix("\x1b[")
        .and_then(|rest| rest.strip_suffix('m'))
    else {
        return sequence.to_string();
    };
    if params.is_empty() || params == "0" {
        return format!("{RESET}{}", hover_default_fg());
    }
    let parts: Vec<&str> = params.split(';').collect();
    let mut rewritten = Vec::new();
    let mut index = 0;
    let mut lifted_fg = false;
    while index < parts.len() {
        let part = parts[index];
        // 1. 【全屏视图】【冒号格式兼容】解析 `38:2::r:g:b` 或 `38:5:idx` 扩展写法
        if part.starts_with("38:") {
            if let Some(rgb) = parse_colon_foreground(part) {
                rewritten.push(lift_fg_sgr(rgb, background));
                lifted_fg = true;
                index += 1;
                continue;
            }
        }
        match part {
            "0" | "" => {
                rewritten.push("0".to_string());
                rewritten.push(hover_default_sgr());
                lifted_fg = true;
                index += 1;
            }
            "38" => match parts.get(index + 1) {
                Some(&"5") if parts.get(index + 2).is_some() => {
                    if let Ok(code) = parts[index + 2].parse::<u8>() {
                        rewritten.push(lift_fg_sgr(xterm256_to_rgb(code), background));
                        lifted_fg = true;
                    } else {
                        rewritten.push(parts[index].to_string());
                    }
                    index += 3;
                }
                Some(&"2") if parts.len() >= index + 5 => {
                    if let (Ok(r), Ok(g), Ok(b)) = (
                        parts[index + 2].parse::<u8>(),
                        parts[index + 3].parse::<u8>(),
                        parts[index + 4].parse::<u8>(),
                    ) {
                        rewritten.push(lift_fg_sgr((r, g, b), background));
                        lifted_fg = true;
                    } else {
                        rewritten.push(parts[index].to_string());
                    }
                    index += 5;
                }
                _ => {
                    rewritten.push(part.to_string());
                    index += 1;
                }
            },
            "2" => {
                // 2. 【全屏视图】【取消暗淡】移除折叠或次要文本上的 dim 效果提升可读性
                rewritten.push("22".to_string());
                index += 1;
            }
            "39" => {
                rewritten.push(hover_default_sgr());
                lifted_fg = true;
                index += 1;
            }
            "48" | "58" => {
                let skip = match parts.get(index + 1) {
                    Some(&"5") => 3,
                    Some(&"2") => 5,
                    _ => 1,
                };
                rewritten.extend(
                    parts[index..(index + skip).min(parts.len())]
                        .iter()
                        .map(|item| (*item).to_string()),
                );
                index += skip;
            }
            other => {
                if let Some(rgb) = ansi16_to_rgb(other) {
                    rewritten.push(lift_fg_sgr(rgb, background));
                    lifted_fg = true;
                } else {
                    rewritten.push(other.to_string());
                }
                index += 1;
            }
        }
    }
    if rewritten.is_empty() {
        return sequence.to_string();
    }
    let mut output = format!("\x1b[{}m", rewritten.join(";"));
    if !lifted_fg && resets_to_default_fg(&rewritten) {
        output.push_str(&hover_default_fg());
    }
    output
}

/// 【全屏视图】【保色相提亮】在 RGB 空间向黑白端点线性混合，不旋转色相角且保证浅色模式可读性。
///
/// 参数:
/// - `color`: 原始语法 token RGB
/// - `background`: 当前终端背景 RGB
///
/// 返回:
/// - 提亮或加深后的 RGB
fn lift_rgb(color: Rgb, background: Rgb) -> Rgb {
    let light = u32::from(background.0) * 299
        + u32::from(background.1) * 587
        + u32::from(background.2) * 114
        > 128_000;
    if light {
        let deepened = blend(color, (0, 0, 0), LIGHT_DEEPEN_AMOUNT);
        readable_color_on(deepened, background, MIN_TEXT_CONTRAST)
    } else {
        let lifted = blend(color, (255, 255, 255), DARK_LIFT_AMOUNT);
        readable_color_on(lifted, background, MIN_TEXT_CONTRAST)
    }
}

/// 【全屏视图】【前景序列生成】输出提亮后 RGB 的 SGR 参数片段。
///
/// 参数:
/// - `color`: 原始前景色 RGB
/// - `background`: 终端背景 RGB
///
/// 返回:
/// - `38;2;r;g;b` 格式字符串
fn lift_fg_sgr(color: Rgb, background: Rgb) -> String {
    let (red, green, blue) = lift_rgb(color, background);
    format!("38;2;{red};{green};{blue}")
}

/// 【全屏视图】【冒号扩展解析】解析 `38:2::r:g:b` / `38:2:r:g:b` / `38:5:idx` 格式。
///
/// 参数:
/// - `part`: 单个冒号分隔的 SGR 参数
///
/// 返回:
/// - 解析出的 RGB，不匹配时为空
fn parse_colon_foreground(part: &str) -> Option<Rgb> {
    let fields: Vec<&str> = part.split(':').collect();
    match fields.get(1).copied() {
        Some("5") => fields
            .get(2)
            .and_then(|value| value.parse::<u8>().ok())
            .map(xterm256_to_rgb),
        Some("2") if fields.len() >= 6 => Some((
            fields[3].parse::<u8>().ok()?,
            fields[4].parse::<u8>().ok()?,
            fields[5].parse::<u8>().ok()?,
        )),
        Some("2") if fields.len() == 5 => Some((
            fields[2].parse::<u8>().ok()?,
            fields[3].parse::<u8>().ok()?,
            fields[4].parse::<u8>().ok()?,
        )),
        _ => None,
    }
}

/// 【全屏视图】【16 色映射】将 ANSI 标准 16 色前景码映射为基础 RGB 供保色相混合使用。
///
/// 参数:
/// - `code`: SGR 前景代码字符串
///
/// 返回:
/// - 对应基础 RGB
fn ansi16_to_rgb(code: &str) -> Option<Rgb> {
    let value = code.parse::<u8>().ok()?;
    let index = match value {
        30..=37 => value - 30,
        90..=97 => value - 90 + 8,
        _ => return None,
    };
    Some(xterm256_to_rgb(index))
}

/// 【全屏视图】【256 色映射】将 xterm 0-255 调色板索引转换为精确 RGB 三元组。
///
/// 参数:
/// - `code`: xterm 颜色索引
///
/// 返回:
/// - 对应 RGB 颜色
fn xterm256_to_rgb(code: u8) -> Rgb {
    const ANSI16: [Rgb; 16] = [
        (36, 40, 48),
        (212, 96, 102),
        (114, 198, 134),
        (224, 182, 98),
        (106, 158, 224),
        (194, 146, 224),
        (98, 196, 212),
        (206, 212, 220),
        (120, 128, 138),
        (238, 122, 128),
        (140, 222, 158),
        (244, 206, 124),
        (134, 184, 244),
        (218, 174, 242),
        (126, 218, 232),
        (242, 245, 248),
    ];
    if code < 16 {
        return ANSI16[usize::from(code)];
    }
    if code >= 232 {
        let gray = 8 + (code - 232) * 10;
        return (gray, gray, gray);
    }
    let idx = code - 16;
    let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
    (level(idx / 36), level((idx % 36) / 6), level(idx % 6))
}

/// 默认悬停前景的 SGR 参数。
fn hover_default_sgr() -> String {
    hover_default_fg()
        .strip_prefix("\x1b[")
        .and_then(|rest| rest.strip_suffix('m'))
        .unwrap_or("22;38;2;255;255;255")
        .to_string()
}

/// 判断改写后的参数是否把前景恢复为默认。
fn resets_to_default_fg(parts: &[String]) -> bool {
    parts.iter().any(|part| part == "0" || part.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::activity_animation::strip_ansi_for_test as plain;

    /// 复位后补回默认悬停色，纯文本不变，不加粗不铺底。
    #[test]
    fn tint_survives_resets_and_keeps_text() {
        let output = tint_line("a\x1b[31mb\x1b[0mc");
        assert!(output.starts_with(&hover_default_fg()));
        assert!(
            output.contains(&format!("{RESET}{}c", hover_default_fg())),
            "{output:?}"
        );
        assert!(output.ends_with(RESET));
        assert_eq!(plain(&output), "abc");
        assert!(!output.contains("\x1b[48;"));
        assert!(!output.contains("\x1b[1m"));
    }

    /// 256 色前景按原色提亮或加深，颜色参数中的 0 不被当成复位。
    #[test]
    fn existing_foreground_is_lifted() {
        let output = tint_line("\x1b[38;5;244mdim");
        assert!(output.contains("38;2;"), "{output:?}");
        assert!(!output.contains("38;5;244"), "{output:?}");
        assert!(!output.contains("\x1b[1m"), "{output:?}");
        assert_eq!(plain(&output), "dim");
        assert!(!rewrite_sequence("\x1b[38;2;0;0;0m", (255, 255, 255)).contains("\x1b[0m"));
    }

    /// 暗淡属性被取消，正文由暗变亮。
    #[test]
    fn dim_intensity_is_cancelled() {
        let output = tint_line("\x1b[2mdim");
        assert!(
            output.contains("\x1b[22m") || output.contains("22;"),
            "{output:?}"
        );
        assert!(!output.contains("\x1b[2m"), "{output:?}");
        assert_eq!(plain(&output), "dim");
    }

    /// 【正文悬停】【样式保留】提亮保留差异背景、下划线颜色及图片协议，不增加背景或粗体。
    /// 参数: 无；返回: 无。
    #[test]
    fn brightening_preserves_existing_backgrounds_and_graphics() {
        let background = "\x1b[48;2;20;46;30m";
        let underline = "\x1b[58;2;0;49;48m";
        let graphics = "\x1b_Ga=p,i=4,p=2\x1b\\";
        let line = format!("{background}{underline}\x1b[2mtext\x1b[0m tail{graphics}");
        let output = tint_line(&line);
        assert_eq!(output.matches("\x1b[48;").count(), 1);
        assert!(output.contains(background));
        assert!(output.contains(underline));
        assert!(output.contains(graphics));
        assert!(!output.contains("\x1b[2m"));
        assert!(!output.contains("\x1b[1m"));
        assert!(output.contains(&format!("{RESET}{} tail", hover_default_fg())));
    }

    /// 【正文悬停】【保色相提亮】暗色和浅色背景下不同语法 token 均保留各自色相，不再被统一替换为单色。
    /// 参数: 无；返回: 无。
    #[test]
    fn hue_preserving_lift_keeps_distinct_syntax_colors_across_themes() {
        let dark_bg = (28, 49, 68);
        let light_bg = (245, 247, 250);
        let cmd = "\x1b[1mRan\x1b[0m \x1b[35m$\x1b[0m \x1b[38;2;156;207;216mcd\x1b[0m \x1b[38;2;166;214;160m'ok'\x1b[0m \x1b[38:2::246:193:119m2\x1b[0m";

        let dark_out = tint_line_on(cmd, dark_bg);
        assert!(!dark_out.contains("\x1b[48;"));
        assert_eq!(plain(&dark_out), "Ran $ cd 'ok' 2");
        let (pr, pg, pb) = lift_rgb(xterm256_to_rgb(5), dark_bg);
        assert!(
            pr > pg && pb > pg,
            "purple should stay purple: {:?}",
            (pr, pg, pb)
        );
        let (gr, gg, gb) = lift_rgb((166, 214, 160), dark_bg);
        assert!(
            gg > gr && gg > gb && gg > 214,
            "green should lift and stay green: {:?}",
            (gr, gg, gb)
        );
        let (yr, yg, yb) = lift_rgb((246, 193, 119), dark_bg);
        assert!(
            yr > yg && yg > yb && yb > 119,
            "yellow should lift and stay warm: {:?}",
            (yr, yg, yb)
        );
        assert!(dark_out.contains(&format!("38;2;{gr};{gg};{gb}")));
        assert!(dark_out.contains(&format!("38;2;{yr};{yg};{yb}")));

        let light_out = tint_line_on(cmd, light_bg);
        let (lgr, lgg, lgb) = lift_rgb((166, 214, 160), light_bg);
        assert!(
            lgg > lgr && lgg > lgb && lgg < 214,
            "light green should deepen: {:?}",
            (lgr, lgg, lgb)
        );
        assert!(light_out.contains(&format!("38;2;{lgr};{lgg};{lgb}")));
    }
}
