//! 帧级文字样式校正：不修改布局、背景或图片协议。

use super::terminal_contrast::{blend, readable_color_on, Rgb, MIN_TEXT_CONTRAST};

#[derive(Default)]
struct TextStyle {
    foreground: Option<Rgb>,
    background: Option<Rgb>,
    background_unknown: bool,
    dim: bool,
    bold: bool,
}

/// 【终端】【色号解析】解析 xterm 256 色，不把 ANSI 主题色假定为固定 RGB。
/// 参数：色号；返回：可确定的 RGB，ANSI 0 到 15 返回空。
pub(crate) fn palette_rgb(index: u8) -> Option<Rgb> {
    if index < 16 {
        return None;
    }
    if index >= 232 {
        let gray = 8 + (index - 232) * 10;
        return Some((gray, gray, gray));
    }
    let cube = index - 16;
    let level = |part: u8| if part == 0 { 0 } else { 55 + part * 40 };
    Some((level(cube / 36), level(cube / 6 % 6), level(cube % 6)))
}

/// 【终端】【颜色输出】真彩直接输出，256 色选择满足对比度且最接近期望色的色号。
/// 参数：前景、背景和颜色能力；返回：独立 SGR 前景序列。
fn foreground_sequence(color: Rgb, background: Rgb, true_color: bool) -> String {
    if true_color {
        return format!("\x1b[38;2;{};{};{}m", color.0, color.1, color.2);
    }
    let index = (16..=255u16)
        .filter_map(|index| {
            let rgb = palette_rgb(index as u8)?;
            if super::terminal_contrast::contrast(rgb, background) < MIN_TEXT_CONTRAST {
                return None;
            }
            let distance = (i32::from(rgb.0) - i32::from(color.0)).pow(2)
                + (i32::from(rgb.1) - i32::from(color.1)).pow(2)
                + (i32::from(rgb.2) - i32::from(color.2)).pow(2);
            Some((distance, index))
        })
        .min()
        .map(|(_, index)| index);
    index.map_or_else(|| "\x1b[39m".into(), |index| format!("\x1b[38;5;{index}m"))
}

impl TextStyle {
    /// 【终端】【样式校正】按原始 SGR 更新状态，暗淡使用可读弱化色而非终端 dim。
    /// 参数：SGR 参数、可选实际默认颜色及颜色能力；返回：原样式与必要的校正序列。
    fn rewrite(
        &mut self,
        parameters: &str,
        defaults: Option<(Rgb, Rgb)>,
        true_color: bool,
    ) -> String {
        let values = parameters
            .split(';')
            .map(|value| value.parse::<u16>().unwrap_or(0))
            .collect::<Vec<_>>();
        let mut kept = Vec::new();
        let mut index = 0;
        let mut cleared_dim = false;
        while index < values.len() {
            let value = values[index];
            match value {
                0 => {
                    *self = Self::default();
                    kept.push(0);
                }
                1 => {
                    self.bold = true;
                    kept.push(1);
                }
                2 => {
                    self.dim = true;
                    cleared_dim = true;
                }
                22 => {
                    self.dim = false;
                    self.bold = false;
                    kept.push(22);
                }
                39 => {
                    self.foreground = None;
                    kept.push(39);
                }
                49 => {
                    self.background = None;
                    self.background_unknown = false;
                    kept.push(49);
                }
                30..=37 | 90..=97 => {
                    self.foreground = None;
                    kept.push(value);
                }
                40..=47 | 100..=107 => {
                    // 1. 【终端】【显式底色】标准黑白底色可确定，其他 ANSI 底色保持终端所有权
                    self.background = match value {
                        40 => Some((0, 0, 0)),
                        47 | 107 => Some((255, 255, 255)),
                        _ => None,
                    };
                    self.background_unknown = self.background.is_none();
                    kept.push(value);
                }
                38 | 48 | 58 => {
                    let count = match values.get(index + 1) {
                        Some(2) => 5,
                        Some(5) => 3,
                        _ => 1,
                    };
                    if index + count > values.len() {
                        return format!("\x1b[{parameters}m");
                    }
                    let color = match values.get(index + 1) {
                        Some(2) => Some((
                            values[index + 2].min(255) as u8,
                            values[index + 3].min(255) as u8,
                            values[index + 4].min(255) as u8,
                        )),
                        Some(5) => palette_rgb(values[index + 2].min(255) as u8),
                        _ => None,
                    };
                    if value == 38 {
                        self.foreground = color;
                    }
                    if value == 48 {
                        self.background = color;
                        self.background_unknown = color.is_none();
                    }
                    kept.extend_from_slice(&values[index..index + count]);
                    index += count;
                    continue;
                }
                _ => kept.push(value),
            }
            index += 1;
        }
        let mut output = String::new();
        if cleared_dim {
            output.push_str(if self.bold { "\x1b[22;1m" } else { "\x1b[22m" });
        }
        if !kept.is_empty() {
            output.push_str(&format!(
                "\x1b[{}m",
                kept.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(";")
            ));
        }
        if let Some((foreground, background)) = defaults.filter(|_| !self.background_unknown) {
            let background = self.background.unwrap_or(background);
            let original = self.foreground.unwrap_or(foreground);
            let desired = if self.dim {
                blend(original, background, 0.6)
            } else {
                original
            };
            let readable = readable_color_on(desired, background, MIN_TEXT_CONTRAST);
            if self.dim || readable != original {
                output.push_str(&foreground_sequence(readable, background, true_color));
            }
        }
        output
    }
}

/// 【终端】【帧文字校正】只改写完整 SGR，OSC、Kitty、光标和布局控制保持逐字一致。
/// 参数：帧文本、实际默认前景背景和颜色能力；返回：可读且不含 dim 的帧文本。
pub(crate) fn correct_ansi_text(
    text: &str,
    defaults: Option<(Rgb, Rgb)>,
    true_color: bool,
) -> String {
    let mut output = String::with_capacity(text.len());
    if let Some((foreground, background)) = defaults {
        let readable = readable_color_on(foreground, background, MIN_TEXT_CONTRAST);
        if readable != foreground {
            output.push_str(&foreground_sequence(readable, background, true_color));
        }
    }
    let mut style = TextStyle::default();
    let mut cursor = 0;
    while cursor < text.len() {
        if text.as_bytes()[cursor] == 0x1b {
            let end = super::terminal_image::escape_sequence_end(text, cursor)
                .max(cursor + 1)
                .min(text.len());
            let sequence = &text[cursor..end];
            if let Some(parameters) = sequence
                .strip_prefix("\x1b[")
                .and_then(|rest| rest.strip_suffix('m'))
                .filter(|parameters| {
                    parameters
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || byte == b';')
                })
            {
                output.push_str(&style.rewrite(parameters, defaults, true_color));
            } else {
                output.push_str(sequence);
            }
            cursor = end;
        } else {
            let end = text[cursor..]
                .find('\x1b')
                .map_or(text.len(), |offset| cursor + offset);
            output.push_str(&text[cursor..end]);
            cursor = end;
        }
    }
    output
}

#[cfg(test)]
#[path = "terminal_text_style_tests.rs"]
mod tests;
