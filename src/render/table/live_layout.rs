use super::{layout::available_width, render_table_with_widths};
use crate::render::markdown_inline::render_table_cell_content;
use std::collections::BTreeMap;

/// 【表格】【流式布局】记录每张长表在各正文宽度下已固定的列宽
/// 表格滚入终端历史后不能回溯改宽，完成时继续沿用这些布局
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct TableLayouts {
    frozen: BTreeMap<(usize, usize), Vec<usize>>,
}

impl TableLayouts {
    /// 【表格】【流式布局】渲染当前表格，超过可变预览预算时固定列宽
    /// 参数: index 为回复内表格序号，lines 为源码，freeze_after 为允许重排列宽的最大行数
    /// 返回: 完整表格文本，不裁剪任何数据行
    pub(super) fn render(
        &mut self,
        index: usize,
        lines: &[String],
        freeze_after: Option<usize>,
    ) -> String {
        let key = (available_width(), index);
        let fixed = self.frozen.get(&key);
        let (rendered, widths) =
            render_table_with_widths(lines, render_table_cell_content, fixed.map(Vec::as_slice));
        if fixed.is_some()
            || !freeze_after.is_some_and(|limit| rendered.lines().count() >= limit.max(1))
        {
            return rendered;
        }
        // 1. 【表格】【流式布局】为后续中文和宽字素保留两列，无法容纳时固定为纵向展示
        let stable = stable_widths(widths, key.0);
        let (rendered, _) =
            render_table_with_widths(lines, render_table_cell_content, Some(&stable));
        // 2. 【表格】【流式布局】固定当前布局，后续数据只增加行数，不改变已输出行的位置
        self.frozen.insert(key, stable);
        rendered
    }
}

/// 【表格】【稳定列宽】为每列保留宽字素空间并保持总宽度预算
/// 参数: widths 为当前列宽，available 为正文净宽
/// 返回: 固定列宽，空列表表示使用纵向展示
fn stable_widths(mut widths: Vec<usize>, available: usize) -> Vec<usize> {
    let budget = available.saturating_sub(widths.len() * 3 + 1);
    if widths.len() * 2 > budget {
        return Vec::new();
    }
    for width in &mut widths {
        *width = (*width).max(2);
    }
    while widths.iter().sum::<usize>() > budget {
        let Some((index, _)) = widths
            .iter()
            .enumerate()
            .filter(|(_, width)| **width > 2)
            .max_by_key(|(_, width)| **width)
        else {
            return Vec::new();
        };
        widths[index] -= 1;
    }
    widths
}
