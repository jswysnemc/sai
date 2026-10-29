use super::super::line::AnsiLine;
use std::time::Duration;

/// 【终端】【微脉冲节拍】单字符形态切换间隔（160毫秒一拍，平滑且不频闪）。
const PULSE_INTERVAL_MS: u128 = 160;

/// 【终端】【微脉冲形态】思考状态呼吸单字符形态循环。
const THINKING_PULSE_FRAMES: [char; 4] = ['◇', '◈', '◆', '◈'];

/// 【终端】【思考微脉冲】根据流逝时间计算当前思考状态单字符形态。
///
/// 参数:
/// - `elapsed`: 任务开始至今的流逝时长
///
/// 返回:
/// - 当前动画帧对应的字符（◇ → ◈ → ◆ → ◈）
pub(super) fn thinking_pulse_icon(elapsed: Duration) -> char {
    // 1. 【终端】【帧号换算】将毫秒时间换算为离散节拍步数
    let step = (elapsed.as_millis() / PULSE_INTERVAL_MS) as usize;

    // 2. 【终端】【循环映射】取模映射到四段形态数组
    THINKING_PULSE_FRAMES[step % THINKING_PULSE_FRAMES.len()]
}

/// 【终端】【活动标记】只替换标题首个可见标记，保留命令、思考正文及全部 ANSI 样式。
/// 参数: `lines` 为已渲染节点，`marker` 为单列动画字符；返回: 无。
pub(super) fn apply_marker(lines: &mut [AnsiLine], marker: &str) {
    let Some(title) = lines.first_mut() else {
        return;
    };
    let mut text = title.as_str().to_string();
    let mut index = 0;
    while index < text.len() {
        let character = text[index..].chars().next().unwrap();
        if character == '\x1b' {
            index = crate::render::terminal_image::escape_sequence_end(&text, index).max(index + 1);
        } else if character.is_whitespace() {
            index += character.len_utf8();
        } else {
            text.replace_range(index..index + character.len_utf8(), marker);
            *title = AnsiLine::new(text);
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【节拍测试】检查四段思考动画及循环边界。
    /// 参数: 无；返回: 无。
    #[test]
    fn test_thinking_pulse_icon_cycle() {
        assert_eq!(thinking_pulse_icon(Duration::from_millis(0)), '◇');
        assert_eq!(thinking_pulse_icon(Duration::from_millis(160)), '◈');
        assert_eq!(thinking_pulse_icon(Duration::from_millis(320)), '◆');
        assert_eq!(thinking_pulse_icon(Duration::from_millis(480)), '◈');
        assert_eq!(thinking_pulse_icon(Duration::from_millis(640)), '◇');
    }

    /// 【终端】【标记测试】工具动画只替换行首符号，正文、命令符号和颜色序列保持原样。
    /// 参数: 无；返回: 无。
    #[test]
    fn tool_pulse_preserves_title_styles_and_body() {
        for frame in [0, 5, 10, 15] {
            let marker =
                crate::render::guide_symbols::pulse(crate::render::guide_symbols::COMMAND, frame);
            let mut lines = vec![
                AnsiLine::new("\x1b[33m•\x1b[0m Running $ echo test".into()),
                AnsiLine::new("unchanged output".into()),
            ];
            apply_marker(&mut lines, &marker);
            assert_eq!(
                lines[0].as_str(),
                format!("\x1b[33m{marker}\x1b[0m Running $ echo test")
            );
            assert_eq!(lines[1].as_str(), "unchanged output");
        }
    }
}
