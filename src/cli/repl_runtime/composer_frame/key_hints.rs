//! 状态栏下方的按键提示行：按当前场景列出可用操作，窄终端从低优先级开始省略。

use crate::cli::repl_chrome::CHROME_FOOTER_SIDE_PAD;
use crate::cli::repl_text::visible_width;
use crate::i18n::text as t;

/// 按键名的颜色。
const KEY_STYLE: &str = "\x1b[38;5;250m";
/// 说明文字与分隔符的颜色。
const LABEL_STYLE: &str = "\x1b[38;5;242m";
/// 需要立即留意的提示（如再按一次退出）。
const NOTICE_STYLE: &str = "\x1b[38;5;214m";
const RESET: &str = "\x1b[0m";
/// 条目间分隔符。
const SEPARATOR: &str = " · ";

/// 生成按键提示所需的界面状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::cli::repl_runtime) struct KeyHintContext {
    /// 模型是否正在运行
    pub(in crate::cli::repl_runtime) streaming: bool,
    /// 是否处于 Ctrl+O 全屏视图
    pub(in crate::cli::repl_runtime) fullscreen: bool,
    /// 需要立即提醒的二次按键确认
    pub(in crate::cli::repl_runtime) notice: Option<KeyNotice>,
}

/// 二次按键确认提示。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::cli::repl_runtime) enum KeyNotice {
    /// 第一次 Ctrl+C 之后
    Exit,
    /// 有输入时第一次 Esc 之后
    ClearInput,
}

impl KeyNotice {
    /// 返回提示文本。
    fn text(self) -> &'static str {
        match self {
            Self::Exit => t("Press Ctrl+C again to exit", "再按一次 Ctrl+C 退出"),
            Self::ClearInput => t("Press Esc again to clear input", "再按一次 Esc 清空输入"),
        }
    }
}

/// 一条按键提示：按键与说明。
struct Hint {
    key: &'static str,
    label: &'static str,
}

/// 构造一条提示。
fn hint(key: &'static str, label: &'static str) -> Hint {
    Hint { key, label }
}

/// 【终端】【按键提示】按场景返回按键条目，越靠前优先级越高。
///
/// 输入为空时列出浏览与模式类按键；开始输入后才出现 Enter、Shift+Enter 等编辑类按键。
/// Ctrl+O 与 Alt+↑↓ 不在这里重复，全屏标题栏已经给出。
///
/// 参数:
/// - `context`: 界面状态
/// - `input_empty`: 输入框是否为空
///
/// 返回:
/// - 按键条目
fn keys_for(context: KeyHintContext, input_empty: bool) -> Vec<Hint> {
    let _ = context.fullscreen;
    match (context.streaming, input_empty) {
        (true, true) => vec![
            hint("Ctrl+C", t("stop", "停止")),
            hint("Ctrl+Z", t("undo queued", "撤回排队")),
            hint("Shift+Tab", t("mode", "切换模式")),
            hint("?", t("shortcuts", "快捷键")),
        ],
        (true, false) => vec![
            hint("Enter", t("queue message", "排队发送")),
            hint("Shift+Enter", t("new line", "换行")),
            hint("Ctrl+C", t("stop", "停止")),
        ],
        (false, true) => vec![
            hint("Shift+Tab", t("mode", "切换模式")),
            hint("↑", t("history", "历史")),
            hint("←", t("session tree", "会话树")),
            hint("?", t("shortcuts", "快捷键")),
        ],
        (false, false) => vec![
            hint("Enter", t("send", "发送")),
            hint("Shift+Enter", t("new line", "换行")),
            hint("Esc Esc", t("clear", "清空")),
            hint("Ctrl+W", t("delete word", "删词")),
        ],
    }
}

/// 【终端】【按键提示】渲染状态栏下方的按键提示行。
///
/// 参数:
/// - `context`: 界面状态
/// - `input_empty`: 输入框是否为空
/// - `panel_open`: 是否有补全或 Shell 面板；面板自带操作说明，此时只保留退出确认
/// - `cols`: 终端列数
///
/// 返回:
/// - 已着色、不超过终端宽度的提示行
pub(in crate::cli::repl_runtime) fn render_key_hints(
    context: KeyHintContext,
    input_empty: bool,
    panel_open: bool,
    cols: usize,
) -> String {
    let pad = CHROME_FOOTER_SIDE_PAD.min(cols.saturating_sub(1) / 2);
    let budget = cols.saturating_sub(pad * 2);
    let separator = format!("{LABEL_STYLE}{SEPARATOR}{RESET}");
    let mut line = String::new();
    let mut used = 0usize;
    // 1. 退出确认优先占据行首，任何场景都显示
    if let Some(notice) = context.notice {
        let notice = notice.text();
        used = visible_width(notice).min(budget);
        line.push_str(&format!("{NOTICE_STYLE}{notice}{RESET}"));
    }
    if !panel_open {
        // 2. 按键条目：从高优先级开始放入，放不下就停止，不截断半个条目
        for item in keys_for(context, input_empty) {
            let plain = format!("{} {}", item.key, item.label);
            let extra = if used == 0 {
                0
            } else {
                SEPARATOR.chars().count()
            };
            if used + extra + visible_width(&plain) > budget {
                break;
            }
            if used > 0 {
                line.push_str(&separator);
            }
            used += extra + visible_width(&plain);
            line.push_str(&format!(
                "{KEY_STYLE}{}{RESET} {LABEL_STYLE}{}{RESET}",
                item.key, item.label
            ));
        }
    }
    format!("{}{line}", " ".repeat(pad))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::activity_animation::strip_ansi_for_test as plain;

    /// 渲染无面板场景并去除样式。
    fn text(context: KeyHintContext, input_empty: bool, cols: usize) -> String {
        plain(&render_key_hints(context, input_empty, false, cols))
    }

    /// 验证输入为空时不出现 Enter 与 Shift+Enter，开始输入后才出现。
    #[test]
    fn enter_hints_appear_only_after_typing() {
        for streaming in [false, true] {
            let context = KeyHintContext {
                streaming,
                ..KeyHintContext::default()
            };
            let empty = text(context, true, 200);
            assert!(!empty.contains("Enter"), "{empty}");
            assert!(empty.contains("Shift+Tab"), "{empty}");
            let typed = text(context, false, 200);
            assert!(
                typed.contains("Enter") && typed.contains("Shift+Enter"),
                "{typed}"
            );
        }
        let running = text(
            KeyHintContext {
                streaming: true,
                ..KeyHintContext::default()
            },
            true,
            200,
        );
        assert!(running.starts_with(&" ".repeat(CHROME_FOOTER_SIDE_PAD.min(99))));
        assert!(running.contains("Ctrl+C"));
        assert!(!running.contains(t("Tips", "技巧")));
    }

    /// 验证补全或 Shell 面板打开时提示行留空，面板自带说明。
    #[test]
    fn panels_suppress_the_hint_row() {
        let line = plain(&render_key_hints(
            KeyHintContext::default(),
            false,
            true,
            200,
        ));
        assert!(line.trim().is_empty(), "{line}");
    }

    /// 验证 Ctrl+O 与 Alt+↑↓ 不出现在提示行，全屏也不提示 PgUp/PgDn。
    #[test]
    fn view_shortcuts_are_left_to_the_fullscreen_header() {
        for fullscreen in [false, true] {
            let line = text(
                KeyHintContext {
                    fullscreen,
                    ..KeyHintContext::default()
                },
                true,
                400,
            );
            assert!(!line.contains("Ctrl+O"), "{line}");
            assert!(!line.contains("Alt+"), "{line}");
            assert!(!line.contains("PgUp"), "{line}");
            assert!(!line.contains("PgDn"), "{line}");
        }
    }

    /// 验证第一次 Ctrl+C 后行首提示再按一次退出，面板打开时同样显示。
    #[test]
    fn exit_notice_leads_the_line() {
        let context = KeyHintContext {
            notice: Some(KeyNotice::Exit),
            ..KeyHintContext::default()
        };
        let notice = t("Press Ctrl+C again to exit", "再按一次 Ctrl+C 退出");
        assert!(text(context, true, 200).trim_start().starts_with(notice));
        let with_panel = plain(&render_key_hints(context, false, true, 200));
        assert_eq!(with_panel.trim(), notice);
        let clear = KeyHintContext {
            notice: Some(KeyNotice::ClearInput),
            ..KeyHintContext::default()
        };
        let clear_text = t("Press Esc again to clear input", "再按一次 Esc 清空输入");
        assert!(text(clear, false, 200).trim_start().starts_with(clear_text));
    }

    /// 验证窄终端按优先级省略，结果不超宽且不截断半个条目。
    #[test]
    fn narrow_terminals_drop_low_priority_hints() {
        for input_empty in [true, false] {
            for cols in [8, 20, 40, 60, 80] {
                let line = render_key_hints(KeyHintContext::default(), input_empty, false, cols);
                assert!(visible_width(&line) <= cols, "{cols}: {line:?}");
                assert!(!plain(&line).trim_end().ends_with('·'), "{cols}: {line:?}");
            }
        }
        let narrow = text(KeyHintContext::default(), false, 30);
        assert!(narrow.contains("Enter"));
        assert!(!narrow.contains(t("delete word", "删词")));
    }
}
