//! 快捷键速查面板：空输入时按 `?` 在输入框上方展开，`?` 或 Esc 收起。

use crate::cli::repl_chrome::CHROME_FOOTER_SIDE_PAD;
use crate::cli::repl_text::visible_width;
use crate::config::PasteImageKey;
use crate::i18n::text as t;

/// 按键颜色。
const KEY_STYLE: &str = "\x1b[38;2;122;162;247m";
/// 分组标题样式。
const TITLE_STYLE: &str = "\x1b[1m";
/// 次要说明颜色。
const DIM_STYLE: &str = "\x1b[38;5;242m";
const RESET: &str = "\x1b[0m";
/// 按键列与说明列之间的空隙。
const KEY_GAP: usize = 2;
/// 分组之间的空隙。
const GROUP_GAP: usize = 4;

/// 一组快捷键。
struct Group {
    title: &'static str,
    items: Vec<(&'static str, &'static str)>,
}

/// 返回图片粘贴键的展示文本。
///
/// 参数:
/// - `key`: 配置的粘贴键
///
/// 返回:
/// - 按键文本
fn paste_key_label(key: PasteImageKey) -> &'static str {
    match key {
        PasteImageKey::CtrlV => "Ctrl+V",
        PasteImageKey::AltV => "Alt+V",
        PasteImageKey::Both => "Ctrl+V / Alt+V",
    }
}

/// 【终端】【快捷键速查】返回全部分组，内容与输入循环中的实际绑定一致。
///
/// 参数:
/// - `paste_key`: 配置的图片粘贴键
///
/// 返回:
/// - 分组列表
fn groups(paste_key: PasteImageKey) -> Vec<Group> {
    vec![
        Group {
            title: t("Compose", "输入"),
            items: vec![
                ("/", t("Commands", "命令")),
                ("@", t("Mention files", "引用文件")),
                ("#", t("Mention skills", "引入 Skills")),
                ("!", t("Shell command", "执行 Shell")),
                ("Shift+Enter", t("New line", "换行")),
                (paste_key_label(paste_key), t("Paste image", "粘贴图片")),
                ("Ctrl+W", t("Delete word", "删除前一个词")),
                ("Esc Esc", t("Clear input", "清空输入")),
            ],
        },
        Group {
            title: t("Session", "会话"),
            items: vec![
                ("Enter", t("Send message", "发送消息")),
                ("Shift+Tab", t("Change mode", "切换权限模式")),
                ("↑ / ↓", t("History", "历史输入")),
                ("←", t("Session tree (empty)", "会话树（空输入）")),
                ("Ctrl+T", t("Fold plan panel", "折叠计划面板")),
                ("Ctrl+C", t("Stop / quit", "停止 / 退出")),
            ],
        },
        Group {
            title: t("While working", "运行中"),
            items: vec![
                ("Enter / Tab", t("Queue message", "排队发送")),
                ("Ctrl+↑", t("Manage queue", "管理队列")),
                ("Ctrl+Z", t("Undo last queued", "撤回队尾")),
                ("Ctrl+Y ×2", t("Clear queue", "清空队列")),
            ],
        },
        Group {
            title: t("Transcript", "会话记录"),
            items: vec![
                ("Ctrl+O", t("Fullscreen view", "全屏视图")),
                ("PgUp", t("Browse output", "浏览输出")),
                ("PgUp / PgDn", t("Scroll (fullscreen)", "翻页（全屏）")),
                ("Alt+↑ / ↓", t("Prev / next turn", "上一条 / 下一条消息")),
                (t("Click", "点击"), t("Fold / unfold", "展开 / 收起")),
                (t("Drag", "拖动"), t("Copy text", "复制文字")),
            ],
        },
    ]
}

/// 渲染后的单个分组：固定宽度的行。
struct Block {
    width: usize,
    lines: Vec<String>,
}

/// 把分组渲染为按键列对齐的文本块。
///
/// 参数:
/// - `group`: 分组
///
/// 返回:
/// - 文本块与显示宽度
fn render_group(group: &Group) -> Block {
    let key_width = group
        .items
        .iter()
        .map(|(key, _)| visible_width(key))
        .max()
        .unwrap_or(0);
    let label_width = group
        .items
        .iter()
        .map(|(_, label)| visible_width(label))
        .max()
        .unwrap_or(0);
    let width = (key_width + KEY_GAP + label_width).max(visible_width(group.title));
    let mut lines = vec![format!("{TITLE_STYLE}{}{RESET}", group.title)];
    for (key, label) in &group.items {
        let padding = key_width + KEY_GAP - visible_width(key);
        lines.push(format!(
            "{KEY_STYLE}{key}{RESET}{}{label}",
            " ".repeat(padding)
        ));
    }
    Block { width, lines }
}

/// 把若干分组上下叠成一列，组间空一行，列宽取最宽的分组。
///
/// 参数:
/// - `blocks`: 同一列的分组
///
/// 返回:
/// - 列文本块
fn stack(blocks: &[&Block]) -> Block {
    let mut lines = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        if index > 0 {
            lines.push(String::new());
        }
        lines.extend(block.lines.iter().cloned());
    }
    Block {
        width: blocks.iter().map(|block| block.width).max().unwrap_or(0),
        lines,
    }
}

/// 把两列并排拼成行，左列补齐到自身宽度。
///
/// 参数:
/// - `left`: 左列
/// - `right`: 右列
///
/// 返回:
/// - 拼好的行
fn side_by_side(left: &Block, right: &Block) -> Vec<String> {
    let height = left.lines.len().max(right.lines.len());
    (0..height)
        .map(|index| {
            let cell = left.lines.get(index).map(String::as_str).unwrap_or("");
            let other = right.lines.get(index).map(String::as_str).unwrap_or("");
            let padding = left.width.saturating_sub(visible_width(cell)) + GROUP_GAP;
            format!("{cell}{}{other}", " ".repeat(padding))
        })
        .collect()
}

/// 【终端】【快捷键速查】按终端宽高排版速查面板。
///
/// 左列为「输入、运行中」，右列为「会话、会话记录」，放得下就两列并排；
/// 太窄时逐组纵向排列。总行数超过 `max_rows` 时截掉末尾并提示放大终端，
/// 避免面板把标题和第一组顶出屏幕。
///
/// 参数:
/// - `cols`: 终端列数
/// - `max_rows`: 面板最多可用的行数
/// - `paste_key`: 配置的图片粘贴键
///
/// 返回:
/// - 面板行，标题行带关闭提示
pub(in crate::cli::repl_runtime) fn render_shortcut_sheet(
    cols: usize,
    max_rows: usize,
    paste_key: PasteImageKey,
) -> Vec<String> {
    let pad = " ".repeat(CHROME_FOOTER_SIDE_PAD.min(cols.saturating_sub(1) / 2));
    let budget = cols.saturating_sub(CHROME_FOOTER_SIDE_PAD * 2).max(1);
    let blocks = groups(paste_key)
        .iter()
        .map(render_group)
        .collect::<Vec<_>>();
    // 1. 两列：左列输入 + 运行中，右列会话 + 会话记录；放不下时单列
    let left = stack(&[&blocks[0], &blocks[2]]);
    let right = stack(&[&blocks[1], &blocks[3]]);
    let body = if left.width + GROUP_GAP + right.width <= budget {
        side_by_side(&left, &right)
    } else {
        stack(&blocks.iter().collect::<Vec<_>>()).lines
    };
    // 2. 标题行带关闭提示，正文与输入框之间的空行由输入框自身的上边距提供
    let title = format!(
        "{TITLE_STYLE}{}{RESET}   {KEY_STYLE}?{RESET} {DIM_STYLE}/{RESET} {KEY_STYLE}Esc{RESET} {DIM_STYLE}{}{RESET}",
        t("Keyboard shortcuts", "快捷键"),
        t("close", "关闭")
    );
    // 3. 标题、空行、正文；超出行数时截掉正文末尾
    let chrome_rows = 2;
    let body_rows = max_rows.saturating_sub(chrome_rows);
    let mut lines = vec![title, String::new()];
    if body.len() > body_rows && body_rows > 0 {
        lines.extend(body.into_iter().take(body_rows.saturating_sub(1)));
        lines.push(format!(
            "{DIM_STYLE}{}{RESET}",
            t("… enlarge the terminal to see all", "… 放大终端查看全部")
        ));
    } else {
        lines.extend(body);
    }
    // 4. 超宽行按显示宽度裁掉，不让面板折行把布局撑乱
    lines
        .into_iter()
        .map(|line| {
            let line = if visible_width(&line) > budget {
                crate::cli::repl_transcript_pager::clip_to_width(&line, budget)
            } else {
                line
            };
            format!("{pad}{line}")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::activity_animation::strip_ansi_for_test as plain;

    /// 在指定语言下渲染并去除样式。
    fn render(language: crate::i18n::Locale, cols: usize, rows: usize) -> Vec<String> {
        crate::i18n::with_locale(language, || {
            render_shortcut_sheet(cols, rows, PasteImageKey::CtrlV)
                .iter()
                .map(|line| plain(line))
                .collect()
        })
    }

    /// 验证中英文在 80 列终端里都两列并排，24 行终端里完整放下且不超宽。
    #[test]
    fn fits_a_standard_terminal_in_both_languages() {
        for language in [crate::i18n::Locale::En, crate::i18n::Locale::Zh] {
            // 24 行终端扣除输入框 6 行与 1 行余量后，面板可用 17 行
            let lines = render(language, 80, 17);
            assert!(lines.len() <= 17, "{language:?}: {} rows", lines.len());
            assert!(
                lines.iter().all(|line| visible_width(line) <= 80),
                "{language:?}"
            );
            let (compose, session) =
                crate::i18n::with_locale(language, || (t("Compose", "输入"), t("Session", "会话")));
            assert!(
                lines
                    .iter()
                    .any(|line| line.contains(compose) && line.contains(session)),
                "{language:?} should be two columns: {lines:#?}"
            );
            assert!(lines.iter().any(|line| line.contains("Shift+Enter")));
            assert!(
                !lines.iter().any(|line| line.contains('…')),
                "{language:?} should not truncate"
            );
        }
    }

    /// 验证窄终端单列排列，行数不足时截掉末尾并给出提示，标题始终在首行。
    #[test]
    fn narrow_or_short_terminals_degrade_gracefully() {
        let narrow = render(crate::i18n::Locale::En, 50, 200);
        assert!(!narrow
            .iter()
            .any(|line| line.contains("Compose") && line.contains("Session")));
        assert!(narrow.iter().all(|line| visible_width(line) <= 50));
        let short = render(crate::i18n::Locale::En, 50, 14);
        assert_eq!(short.len(), 14);
        assert!(short[0].contains("Keyboard shortcuts"));
        assert!(short
            .iter()
            .any(|line| line.contains("enlarge the terminal")));
        assert!(short[0].contains("Esc"));
        for line in render(crate::i18n::Locale::Zh, 20, 40) {
            assert!(visible_width(&line) <= 20, "{line:?}");
        }
    }

    /// 验证按键列对齐、粘贴键随配置变化。
    #[test]
    fn items_are_aligned_and_reflect_config() {
        let lines = crate::i18n::with_locale(crate::i18n::Locale::En, || {
            render_shortcut_sheet(60, 200, PasteImageKey::AltV)
                .iter()
                .map(|line| plain(line))
                .collect::<Vec<_>>()
        });
        let text = lines.join("\n");
        assert!(text.contains("Alt+V"));
        assert!(!text.contains("Ctrl+V"));
        let label_col = |prefix: &str| {
            let line = lines
                .iter()
                .find(|line| line.trim_start().starts_with(prefix))
                .unwrap();
            let start = line.len() - line.trim_start().len() + prefix.len();
            start + line[start..].len() - line[start..].trim_start().len()
        };
        assert_eq!(label_col("Enter "), label_col("Shift+Tab"));
    }
}
