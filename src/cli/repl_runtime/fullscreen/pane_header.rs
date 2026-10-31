//! 右侧面板共用的独立品牌标识区，与分组标题和滚动内容分开。

use super::sidebar_widgets::{columns, highlight};
use crate::render::brand_logo::{logo_lines, MARK_STYLE};

/// 【终端侧栏】【标识高度】正常尺寸显示完整标志，矮屏退回单行品牌名称。
/// 参数: rows 为面板高度；返回: 标识区行数。
pub(super) fn height(rows: usize) -> usize {
    if rows >= 12 {
        3
    } else {
        1
    }
}

/// 【终端侧栏】【关闭行号】关闭按钮与品牌名称垂直居中对齐。
/// 参数: rows 为面板高度；返回: 关闭按钮屏幕行号。
pub(super) fn close_row(rows: usize) -> u16 {
    (height(rows) / 2) as u16
}

/// 【终端侧栏】【关闭命中】只命中按钮文字，不占用分组标题或右侧留白。
/// 参数: cols/rows 为面板终点及高度，column/row 为指针坐标；返回: 是否命中关闭按钮。
pub(super) fn close_at(cols: u16, rows: usize, column: u16, row: u16) -> bool {
    row == close_row(rows) && column >= cols.saturating_sub(4) && column < cols.saturating_sub(1)
}

/// 【终端侧栏】【品牌标识】复用终端原生 logo，名称和关闭按钮另占固定区域，不增加背景。
/// 参数: title 为可选面板名称，width 为净宽，rows 为高度，hovered 为关闭悬停；返回: 标识区各行。
pub(super) fn render(title: &str, width: usize, rows: usize, hovered: bool) -> Vec<String> {
    let count = height(rows);
    let center = count / 2;
    let mark = logo_lines(MARK_STYLE);
    let button = if hovered {
        highlight("[x]", false)
    } else {
        "[x]".into()
    };
    (0..count)
        .map(|index| {
            let logo = if count == 3 { mark[index].as_str() } else { "" };
            let name = if index == center {
                format!(
                    "{logo} \x1b[1msai\x1b[22m{}",
                    if title.is_empty() {
                        String::new()
                    } else {
                        format!("  {title}")
                    }
                )
            } else {
                logo.to_string()
            };
            columns(&name, if index == center { &button } else { "" }, width)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::ansi_clip::strip_ansi;

    /// 【侧栏测试】【标识几何】完整 logo 和关闭按钮不占用分组标题，矮屏仍可关闭。
    /// 参数: 无；返回: 无。
    #[test]
    fn logo_and_close_button_share_a_dedicated_header() {
        let rows = render("", 37, 32, true);
        assert_eq!(rows.len(), 3);
        assert!(strip_ansi(&rows[0]).starts_with("╭──╴"));
        assert!(strip_ansi(&rows[1]).contains("sai"));
        assert!(strip_ansi(&rows[2]).starts_with("╶──╯"));
        assert!(rows[1].contains("\x1b[7m[x]"));
        assert!(rows.iter().all(|row| !row.contains("\x1b[48;")));
        assert!(close_at(160, 32, 157, 1));
        assert!(!close_at(160, 32, 157, 0));
        assert_eq!(render("Review", 20, 8, false).len(), 1);
        assert!(close_at(40, 8, 37, 0));
    }
}
