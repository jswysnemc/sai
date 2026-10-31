//! 右侧面板共用的独立品牌标识区，与分组标题和滚动内容分开。

use super::sidebar_widgets::columns;
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

/// 【终端侧栏】【品牌标识】复用终端原生 logo，标识区不重复绘制关闭控件。
/// 参数: title 为可选面板名称，width 为净宽，rows 为高度；返回: 标识区各行。
pub(super) fn render(title: &str, width: usize, rows: usize) -> Vec<String> {
    let count = height(rows);
    let center = count / 2;
    let mark = logo_lines(MARK_STYLE);
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
            columns(&name, "", width)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::ansi_clip::strip_ansi;

    /// 【侧栏测试】【标识几何】完整 logo 不占用分组标题，正常和矮屏都不显示重复按钮。
    /// 参数: 无；返回: 无。
    #[test]
    fn logo_header_has_no_duplicate_close_button() {
        let rows = render("", 37, 32);
        assert_eq!(rows.len(), 3);
        assert!(strip_ansi(&rows[0]).starts_with("╭──╴"));
        assert!(strip_ansi(&rows[1]).contains("sai"));
        assert!(strip_ansi(&rows[2]).starts_with("╶──╯"));
        assert!(rows.iter().all(|row| !row.contains("[x]")));
        assert!(rows.iter().all(|row| !row.contains("\x1b[48;")));
        assert_eq!(render("Review", 20, 8).len(), 1);
    }
}
