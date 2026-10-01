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
                ("Ctrl+← / →", t("Move by word", "按词移动")),
                ("Esc Esc", t("Clear input", "清空输入")),
            ],
        },
        Group {
            title: t("Session", "会话"),
            items: vec![
                ("Enter", t("Send message", "发送消息")),
                ("Shift+Tab", t("Change mode", "切换权限模式")),
                ("↑ / ↓", t("History", "历史输入")),
                ("←", t("Session tree (empty prompt)", "会话树（空输入）")),
                ("Ctrl+T", t("Fold plan panel", "折叠计划面板")),
                ("Ctrl+L", t("Redraw screen", "重绘屏幕")),
                ("Ctrl+C", t("Stop / quit", "停止 / 退出")),
            ],
        },
        Group {
            title: t("While working", "运行中"),
            items: vec![
                ("Enter / Tab", t("Queue message", "排队发送")),
                ("Ctrl+↑", t("Manage queue", "管理队列")),
                ("Ctrl+Z", t("Undo last queued", "撤回队尾")),
                ("Ctrl+Y Ctrl+Y", t("Clear queue", "清空队列")),
            ],
        },
        Group {
            title: t("Transcript", "会话记录"),
            items: vec![
                ("Ctrl+O", t("Fullscreen view", "全屏视图")),
                ("PgUp", t("Browse output", "浏览输出")),
                ("PgUp / PgDn", t("Scroll (fullscreen)", "翻页（全屏）")),
                ("Alt+↑ / ↓", t("Previous / next message", "上一条 / 下一条消息")),
                ("Ctrl+Home / End", t("Top / latest", "顶部 / 最新")),
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
        lines.push(format!("{KEY_STYLE}{key}{RESET}{}{label}", " ".repeat(padding)));
    }
    Block { width, lines }
}

/// 把若干分组并排拼成行，每列补齐到该列在所有排里的最大宽度，上下两排对齐。
///
/// 参数:
/// - `blocks`: 同一排的分组
/// - `widths`: 每列的统一宽度
///
/// 返回:
/// - 拼好的行
fn join_row(blocks: &[&Block], widths: &[usize]) -> Vec<String> {
    let height = blocks.iter().map(|block| block.lines.len()).max().unwrap_or(0);
    (0..height)
        .map(|index| {
            let mut line = String::new();
            for (position, block) in blocks.iter().enumerate() {
                let cell = block.lines.get(index).map(String::as_str).unwrap_or("");
                line.push_str(cell);
                if position + 1 < blocks.len() {
                    let column = widths.get(position).copied().unwrap_or(block.width);
                    let padding = column.saturating_sub(visible_width(cell)) + GROUP_GAP;
                    line.push_str(&" ".repeat(padding));
                }
            }
            line
        })
        .collect()
}

/// 【终端】【快捷键速查】按终端宽度排版速查面板，放不下两列时逐组纵向排列。
///
/// 参数:
/// - `cols`: 终端列数
/// - `paste_key`: 配置的图片粘贴键
///
/// 返回:
/// - 面板行，末尾带关闭提示
pub(in crate::cli::repl_runtime) fn render_shortcut_sheet(
    cols: usize,
    paste_key: PasteImageKey,
) -> Vec<String> {
    let pad = " ".repeat(CHROME_FOOTER_SIDE_PAD.min(cols.saturating_sub(1) / 2));
    let budget = cols.saturating_sub(CHROME_FOOTER_SIDE_PAD * 2).max(1);
    let blocks = groups(paste_key).iter().map(render_group).collect::<Vec<_>>();
    // 1. 能放下就两组一排，否则一组一排；每列取所有排中的最大宽度
    let widths = (0..2)
        .map(|column| {
            blocks
                .iter()
                .skip(column)
                .step_by(2)
                .map(|block| block.width)
                .max()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    let per_row = if widths.iter().sum::<usize>() + GROUP_GAP <= budget {
        2
    } else {
        1
    };
    let mut lines = vec![
        format!("{TITLE_STYLE}{}{RESET}", t("Keyboard shortcuts", "快捷键")),
        String::new(),
    ];
    for (index, row) in blocks.chunks(per_row).enumerate() {
        if index > 0 {
            lines.push(String::new());
        }
        lines.extend(join_row(&row.iter().collect::<Vec<_>>(), &widths));
    }
    lines.push(String::new());
    lines.push(format!(
        "{KEY_STYLE}?{RESET} {DIM_STYLE}/{RESET} {KEY_STYLE}Esc{RESET} {DIM_STYLE}{}{RESET}",
        t("close", "关闭")
    ));
    lines.push(String::new());
    // 2. 超宽行按显示宽度裁掉，不让面板折行把布局撑乱
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

    /// 验证宽终端两组并排，窄终端逐组排列，且都不超宽。
    #[test]
    fn layout_adapts_to_width() {
        let wide = render_shortcut_sheet(140, PasteImageKey::CtrlV);
        let narrow = render_shortcut_sheet(50, PasteImageKey::CtrlV);
        assert!(narrow.len() > wide.len());
        let compose = t("Compose", "输入");
        let session = t("Session", "会话");
        assert!(wide.iter().any(|line| {
            let line = plain(line);
            line.contains(compose) && line.contains(session)
        }));
        for (cols, lines) in [(140, &wide), (50, &narrow), (20, &render_shortcut_sheet(20, PasteImageKey::Both))] {
            for line in lines.iter() {
                assert!(visible_width(line) <= cols, "{cols}: {line:?}");
            }
        }
    }

    /// 验证两排分组的第二列起始位置一致。
    #[test]
    fn columns_line_up_across_rows() {
        let lines = render_shortcut_sheet(140, PasteImageKey::CtrlV)
            .iter()
            .map(|line| plain(line))
            .collect::<Vec<_>>();
        let start = |title: &str| {
            lines
                .iter()
                .find_map(|line| line.find(title).map(|at| visible_width(&line[..at])))
                .unwrap()
        };
        assert_eq!(start(t("Session", "会话")), start(t("Transcript", "会话记录")));
    }

    /// 验证按键列对齐、粘贴键随配置变化，末尾给出关闭提示。
    #[test]
    fn items_are_aligned_and_reflect_config() {
        let lines = render_shortcut_sheet(60, PasteImageKey::AltV)
            .iter()
            .map(|line| plain(line))
            .collect::<Vec<_>>();
        let text = lines.join("\n");
        assert!(text.contains("Alt+V"));
        assert!(!text.contains("Ctrl+V"));
        let enter = lines.iter().find(|line| line.trim_start().starts_with("Enter ")).unwrap();
        let tab = lines.iter().find(|line| line.trim_start().starts_with("Shift+Tab")).unwrap();
        let label_col = |line: &str| {
            let trimmed = line.trim_start();
            let key_end = trimmed.find("  ").unwrap();
            line.len() - trimmed.len() + key_end + trimmed[key_end..].len() - trimmed[key_end..].trim_start().len()
        };
        assert_eq!(label_col(enter), label_col(tab));
        assert!(lines.iter().rev().nth(1).unwrap().contains("Esc"));
    }
}
