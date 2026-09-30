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

/// 输入框下方正在显示的补全面板。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::cli::repl_runtime) enum HintPanel {
    /// 没有补全面板
    #[default]
    None,
    /// 斜杠命令或 @/# 引用候选
    Completion,
    /// `!` Shell 模式提示
    Shell,
}

/// 生成按键提示所需的界面状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::cli::repl_runtime) struct KeyHintContext {
    /// 模型是否正在运行
    pub(in crate::cli::repl_runtime) streaming: bool,
    /// 是否处于 Ctrl+O 全屏视图
    pub(in crate::cli::repl_runtime) fullscreen: bool,
    /// 第一次 Ctrl+C 之后、退出窗口尚未过期
    pub(in crate::cli::repl_runtime) exit_pending: bool,
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

/// 【终端】【按键提示】按场景返回提示条目，越靠前优先级越高。
///
/// 参数:
/// - `context`: 界面状态
/// - `panel`: 当前补全面板
/// - `input_empty`: 输入框是否为空
///
/// 返回:
/// - 提示条目
fn hints_for(context: KeyHintContext, panel: HintPanel, input_empty: bool) -> Vec<Hint> {
    let mut hints = match panel {
        HintPanel::Completion => vec![
            hint("↑↓", t("select", "选择")),
            hint("Tab", t("complete", "补全")),
            hint("Enter", t("confirm", "确认")),
            hint("Esc", t("dismiss", "收起")),
        ],
        HintPanel::Shell => vec![
            hint("Enter", t("run shell command", "执行 Shell 命令")),
            hint("Backspace", t("leave shell mode", "删去 ! 退出")),
        ],
        HintPanel::None if context.streaming => vec![
            hint("Ctrl+C", t("stop", "停止")),
            hint("Enter", t("queue message", "排队发送")),
            hint("Tab", t("queue", "入队")),
            hint("Ctrl+Z", t("undo queued", "撤回排队")),
            hint("Shift+Tab", t("mode", "切换模式")),
        ],
        HintPanel::None if input_empty => vec![
            hint("Enter", t("send", "发送")),
            hint("/", t("commands", "命令")),
            hint("@", t("mention file", "引用文件")),
            hint("!", t("shell", "Shell")),
            hint("Shift+Tab", t("mode", "切换模式")),
            hint("↑", t("history", "历史")),
            hint("←", t("session tree", "会话树")),
        ],
        HintPanel::None => vec![
            hint("Enter", t("send", "发送")),
            hint("Shift+Enter", t("new line", "换行")),
            hint("Esc Esc", t("clear", "清空")),
            hint("Ctrl+W", t("delete word", "删词")),
            hint("Shift+Tab", t("mode", "切换模式")),
        ],
    };
    // 视图切换排在场景前三条之后：80 列终端也能看到；全屏额外给出浏览键
    let view = if context.fullscreen {
        vec![
            hint("Ctrl+O", t("exit fullscreen", "退出全屏")),
            hint("PgUp/PgDn", t("scroll", "翻页")),
            hint("Alt+↑↓", t("messages", "切换消息")),
        ]
    } else {
        vec![hint("Ctrl+O", t("fullscreen", "全屏"))]
    };
    let at = hints.len().min(3);
    hints.splice(at..at, view);
    hints
}

/// 【终端】【按键提示】渲染状态栏下方的按键提示行。
///
/// 参数:
/// - `context`: 界面状态
/// - `panel`: 当前补全面板
/// - `input_empty`: 输入框是否为空
/// - `cols`: 终端列数
///
/// 返回:
/// - 已着色、不超过终端宽度的提示行
pub(in crate::cli::repl_runtime) fn render_key_hints(
    context: KeyHintContext,
    panel: HintPanel,
    input_empty: bool,
    cols: usize,
) -> String {
    let pad = CHROME_FOOTER_SIDE_PAD.min(cols.saturating_sub(1) / 2);
    let budget = cols.saturating_sub(pad * 2);
    // 1. 退出确认优先占据行首，其余提示照常跟在后面
    let mut parts = Vec::new();
    let mut used = 0usize;
    if context.exit_pending {
        let notice = t("Press Ctrl+C again to exit", "再按一次 Ctrl+C 退出");
        used = visible_width(notice).min(budget);
        parts.push(format!("{NOTICE_STYLE}{notice}{RESET}"));
    }
    // 2. 从高优先级开始放入，放不下就停止，保证不会在中间截断半个条目
    for item in hints_for(context, panel, input_empty) {
        let plain = format!("{} {}", item.key, item.label);
        let extra = if parts.is_empty() { 0 } else { SEPARATOR.chars().count() };
        let width = visible_width(&plain) + extra;
        if used + width > budget {
            break;
        }
        used += width;
        parts.push(format!(
            "{KEY_STYLE}{}{RESET} {LABEL_STYLE}{}{RESET}",
            item.key, item.label
        ));
    }
    format!(
        "{}{}",
        " ".repeat(pad),
        parts.join(&format!("{LABEL_STYLE}{SEPARATOR}{RESET}"))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::activity_animation::strip_ansi_for_test as plain;

    /// 验证空闲、运行与补全面板给出不同提示。
    #[test]
    fn hints_follow_the_current_scene() {
        let idle = plain(&render_key_hints(KeyHintContext::default(), HintPanel::None, true, 200));
        assert!(idle.contains("Enter"));
        assert!(idle.contains("Ctrl+O"));
        let running = plain(&render_key_hints(
            KeyHintContext {
                streaming: true,
                ..KeyHintContext::default()
            },
            HintPanel::None,
            true,
            200,
        ));
        assert!(running.starts_with(&" ".repeat(CHROME_FOOTER_SIDE_PAD.min(99))));
        assert!(running.contains("Ctrl+C"));
        assert_ne!(idle, running);
        let completion = plain(&render_key_hints(
            KeyHintContext::default(),
            HintPanel::Completion,
            false,
            200,
        ));
        assert!(completion.contains("↑↓") && completion.contains("Esc"));
    }

    /// 验证第一次 Ctrl+C 后行首提示再按一次退出。
    #[test]
    fn exit_notice_leads_the_line() {
        let text = plain(&render_key_hints(
            KeyHintContext {
                exit_pending: true,
                ..KeyHintContext::default()
            },
            HintPanel::None,
            true,
            200,
        ));
        let notice = t("Press Ctrl+C again to exit", "再按一次 Ctrl+C 退出");
        assert!(text.trim_start().starts_with(notice), "{text}");
    }

    /// 验证全屏视图给出退出全屏提示。
    #[test]
    fn fullscreen_hints_mention_exit() {
        let text = plain(&render_key_hints(
            KeyHintContext {
                fullscreen: true,
                ..KeyHintContext::default()
            },
            HintPanel::None,
            false,
            400,
        ));
        assert!(text.contains(t("exit fullscreen", "退出全屏")), "{text}");
    }

    /// 验证窄终端按优先级省略，结果不超宽且不截断半个条目。
    #[test]
    fn narrow_terminals_drop_low_priority_hints() {
        for cols in [8, 20, 40, 60] {
            let text = render_key_hints(KeyHintContext::default(), HintPanel::None, true, cols);
            assert!(visible_width(&text) <= cols, "{cols}: {text:?}");
            assert!(!plain(&text).trim_end().ends_with('·'), "{cols}: {text:?}");
        }
        let narrow = plain(&render_key_hints(KeyHintContext::default(), HintPanel::None, true, 30));
        assert!(narrow.contains("Enter"));
        assert!(!narrow.contains(t("session tree", "会话树")));
        // 80 列时视图切换键仍可见
        let common = plain(&render_key_hints(KeyHintContext::default(), HintPanel::None, true, 80));
        assert!(common.contains("Ctrl+O"), "{common}");
    }
}
