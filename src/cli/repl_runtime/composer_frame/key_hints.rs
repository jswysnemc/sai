//! 状态栏下方的按键提示行：按当前场景列出可用操作，接入 PanelPalette 自适应明暗背景。

use crate::cli::repl_chrome::CHROME_FOOTER_SIDE_PAD;
use crate::cli::repl_text::visible_width;
use crate::i18n::text as t;
use crate::render::panel_palette::{fg, PanelPalette};

const RESET: &str = "\x1b[0m";
/// 条目间分隔符。
const SEPARATOR: &str = " · ";

/// 底部提示行所处的焦点模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::cli::repl_runtime) enum ActiveBottomMode {
    /// 常规输入与状态浏览模式
    #[default]
    Normal,
    /// 引用面板或隐藏顶栏的单条斜杠补全候选模式
    Completion,
    /// 用户消息队列管理面板焦点模式
    QueuePanel,
    /// 主/子智能体切换面板焦点模式
    AgentPanel,
}

/// 生成按键提示所需的界面状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::cli::repl_runtime) struct KeyHintContext {
    /// 模型是否正在运行
    pub(in crate::cli::repl_runtime) streaming: bool,
    /// 是否处于 Ctrl+O 全屏视图
    pub(in crate::cli::repl_runtime) fullscreen: bool,
    /// 需要立即提醒的二次按键确认
    pub(in crate::cli::repl_runtime) notice: Option<KeyNotice>,
    /// 当前激活的底部面板或补全状态
    pub(in crate::cli::repl_runtime) mode: ActiveBottomMode,
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
    /// 【终端】【确认提示】返回二次按键提示文本。
    /// 参数: `self` 为通知枚举；返回: 当前语言下的静态提示字符串。
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

/// 【终端】【提示构造】构造单条按键提示项。
/// 参数: `key` 为键名，`label` 为动作说明；返回: 提示结构体。
fn hint(key: &'static str, label: &'static str) -> Hint {
    Hint { key, label }
}

/// 【终端】【按键提示】按焦点面板与输入场景返回有序按键条目，越靠前优先级越高。
/// 参数: `context` 为界面状态，`input_empty` 为输入框是否为空；返回: 按键条目列表。
fn keys_for(context: KeyHintContext, input_empty: bool) -> Vec<Hint> {
    match context.mode {
        ActiveBottomMode::Completion => {
            return vec![
                hint("↑↓", t("select", "选择")),
                hint("Tab/Enter", t("apply", "补全")),
                hint("Esc", t("dismiss", "关闭")),
            ];
        }
        ActiveBottomMode::QueuePanel => {
            return vec![
                hint("↑↓", t("select", "选择")),
                hint("k/j", t("reorder", "调序")),
                hint("Tab", t("timing", "时机")),
                hint("Enter", t("edit", "编辑")),
                hint("Esc", t("close", "关闭")),
            ];
        }
        ActiveBottomMode::AgentPanel => {
            return vec![
                hint("↑↓", t("select", "选择")),
                hint("Enter", t("switch view", "切换视图")),
                hint("Esc", t("close", "关闭")),
            ];
        }
        ActiveBottomMode::Normal => {}
    }

    let mut keys = match (context.streaming, input_empty) {
        (true, true) => vec![
            hint("Ctrl+C", t("stop", "停止")),
            hint("Ctrl+↑", t("queue manager", "队列管理")),
            hint("Ctrl+Z", t("undo queued", "撤回排队")),
            hint("Shift+Tab", t("mode", "切换模式")),
            hint("?", t("shortcuts", "快捷键")),
        ],
        (true, false) => vec![
            hint("Enter", t("next turn", "下轮接力")),
            hint("Ctrl+Enter", t("steer", "次回插话")),
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
    };
    if !context.fullscreen && !context.streaming && input_empty {
        keys.insert(
            1,
            hint("Shift+mouse", t("terminal select/scroll", "终端选择/滚动")),
        );
    }
    keys
}

/// 【终端】【按键提示】按当前终端背景渲染状态栏下方的按键提示行。
/// 参数: `context` 为界面状态，`input_empty` 为输入是否为空，`panel_open` 为是否已有自带说明的面板，`cols` 为列宽；返回: 已着色提示行。
pub(in crate::cli::repl_runtime) fn render_key_hints(
    context: KeyHintContext,
    input_empty: bool,
    panel_open: bool,
    cols: usize,
) -> String {
    render_key_hints_with_palette(
        context,
        input_empty,
        panel_open,
        cols,
        &PanelPalette::current(),
    )
}

/// 【终端】【色板渲染】使用指定面板色板渲染按键提示行，保证主键名在明暗终端均重于说明词。
/// 参数: `context` 为界面状态，`input_empty` 为是否空输入，`panel_open` 为是否抑制常规条目，`cols` 为列宽，`palette` 为色板；返回: 提示行。
fn render_key_hints_with_palette(
    context: KeyHintContext,
    input_empty: bool,
    panel_open: bool,
    cols: usize,
    palette: &PanelPalette,
) -> String {
    let pad = CHROME_FOOTER_SIDE_PAD.min(cols.saturating_sub(1) / 2);
    let budget = cols.saturating_sub(pad * 2);
    let key_style = fg(palette.text);
    let label_style = fg(palette.muted);
    let notice_style = fg(palette.warning);
    let separator = format!("{label_style}{SEPARATOR}{RESET}");
    let mut line = String::new();
    let mut used = 0usize;
    // 1. 【终端】【退出确认】退出或清空确认优先占据行首，窄终端按显示列裁剪
    if let Some(notice) = context.notice {
        let notice = clip_notice(notice.text(), budget);
        used = visible_width(&notice);
        if !notice.is_empty() {
            line.push_str(&format!("{notice_style}{notice}{RESET}"));
        }
    }
    if !panel_open {
        // 2. 【终端】【分段填充】按优先级依次放入按键与说明，空间不足时整段停止不截半词
        for item in keys_for(context, input_empty) {
            let plain = format!("{} {}", item.key, item.label);
            let extra = if used == 0 {
                0
            } else {
                visible_width(SEPARATOR)
            };
            if used + extra + visible_width(&plain) > budget {
                break;
            }
            if used > 0 {
                line.push_str(&separator);
            }
            used += extra + visible_width(&plain);
            line.push_str(&format!(
                "{key_style}{}{RESET} {label_style}{}{RESET}",
                item.key, item.label
            ));
        }
    }
    format!("{}{line}", " ".repeat(pad))
}

/// 【终端】【提示裁剪】把确认提示裁到指定显示列数。
/// 参数: `text` 为纯文本提示，`width` 为最大显示列数；返回: 不超过列数的文本。
fn clip_notice(text: &str, width: usize) -> String {
    if width == 0 || text.is_empty() {
        return String::new();
    }
    if visible_width(text) <= width {
        return text.to_string();
    }
    const ELLIPSIS: &str = "...";
    const ELLIPSIS_WIDTH: usize = 3;
    if width <= ELLIPSIS_WIDTH {
        return ".".repeat(width);
    }
    let mut output = String::new();
    let mut used = 0usize;
    for ch in text.chars() {
        let char_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if used.saturating_add(char_width) + ELLIPSIS_WIDTH > width {
            break;
        }
        output.push(ch);
        used = used.saturating_add(char_width);
    }
    output.push_str(ELLIPSIS);
    output
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

    /// 验证自带操作顶栏的面板打开时提示行留空。
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

    /// 【按键提示测试】【专属面板与浅色对比度】验证补全、队列、子代理专属提示及浅色终端键名层次。
    #[test]
    fn dedicated_modes_and_light_palette_keep_key_hierarchy() {
        let completion = text(
            KeyHintContext {
                mode: ActiveBottomMode::Completion,
                ..KeyHintContext::default()
            },
            false,
            200,
        );
        assert!(completion.contains("Tab/Enter") && completion.contains("Esc"));
        let queue = text(
            KeyHintContext {
                mode: ActiveBottomMode::QueuePanel,
                ..KeyHintContext::default()
            },
            true,
            200,
        );
        assert!(queue.contains("k/j") && queue.contains("Tab"));
        let agent = text(
            KeyHintContext {
                mode: ActiveBottomMode::AgentPanel,
                ..KeyHintContext::default()
            },
            true,
            200,
        );
        assert!(agent.contains("Enter") && agent.contains("Esc"));

        let light = PanelPalette::for_background((250, 250, 250));
        assert!(light.text.0 < light.muted.0);
        let rendered =
            render_key_hints_with_palette(KeyHintContext::default(), true, false, 120, &light);
        assert!(rendered.contains(&fg(light.text)));
        assert!(rendered.contains(&fg(light.muted)));
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

    /// 【终端】【按键提示】退出确认在窄终端和中文文案下也不超出列数。
    #[test]
    fn exit_notice_stays_within_narrow_columns() {
        let context = KeyHintContext {
            notice: Some(KeyNotice::Exit),
            ..KeyHintContext::default()
        };
        for language in [crate::i18n::Locale::En, crate::i18n::Locale::Zh] {
            crate::i18n::with_locale(language, || {
                for cols in [1usize, 8, 16, 24, 40, 80] {
                    let line = render_key_hints(context, true, false, cols);
                    assert!(
                        visible_width(&line) <= cols,
                        "{language:?} {cols}: {}",
                        plain(&line)
                    );
                }
            });
        }
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
