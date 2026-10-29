use super::{
    formatting::{swatch, FREE, RESERVED},
    ContextOverview,
};

/// 【上下文】【分类分配】单元足够时每个非零分类至少占一格，其余按最大余数分配，总量保持不变。
/// 参数: 分类权重和单元预算；返回: 每类单元数。
fn allocate(weights: &[usize], cells: usize) -> Vec<usize> {
    let nonzero = weights.iter().filter(|weight| **weight > 0).count();
    if nonzero == 0 || cells < nonzero {
        return proportional(weights, cells);
    }
    // 1. 先为每个非零分类保底一格，避免小分类在图上消失
    let floor = weights
        .iter()
        .map(|weight| usize::from(*weight > 0))
        .collect::<Vec<_>>();
    // 2. 剩余单元继续按权重比例分配
    proportional(weights, cells - nonzero)
        .into_iter()
        .zip(floor)
        .map(|(count, floor)| count + floor)
        .collect()
}

/// 【上下文】【比例分配】按最大余数把单元预算分给各分类。
/// 参数: 分类权重和单元预算；返回: 每类单元数。
fn proportional(weights: &[usize], cells: usize) -> Vec<usize> {
    let total = weights.iter().map(|value| *value as u128).sum::<u128>();
    if total == 0 {
        return vec![0; weights.len()];
    }
    let mut counts = weights
        .iter()
        .map(|weight| (*weight as u128 * cells as u128 / total) as usize)
        .collect::<Vec<_>>();
    let mut remainder = weights
        .iter()
        .enumerate()
        .map(|(index, weight)| (index, *weight as u128 * cells as u128 % total))
        .collect::<Vec<_>>();
    remainder.sort_by_key(|(index, weight)| (std::cmp::Reverse(*weight), *index));
    for (index, _) in remainder
        .into_iter()
        .take(cells.saturating_sub(counts.iter().sum()))
    {
        counts[index] += 1;
    }
    counts
}

/// 【上下文】【分段柱图】用半字符分段提高小比例分辨率，底部纹理表示压缩预留。
/// 参数: 数据、柱数、高度和着色开关；返回: 固定宽度图表行。
pub(super) fn render(
    view: &ContextOverview,
    columns: usize,
    rows: usize,
    styled: bool,
) -> Vec<String> {
    let columns = columns.max(1);
    let cells = columns * rows * 2;
    let window = view.window.max(1);
    let used = view.used.min(window);
    let occupied = ((used as u128 * cells as u128).div_ceil(window as u128) as usize).min(cells);
    let (_, reserved) = view.free_and_reserved();
    let reserved_cells =
        ((reserved as u128 * cells as u128 / window as u128) as usize).min(cells - occupied);
    let counts = allocate(
        &view
            .categories
            .iter()
            .map(|category| category.tokens)
            .collect::<Vec<_>>(),
        occupied,
    );
    let mut colors = Vec::with_capacity(cells);
    for (category, count) in view.categories.iter().zip(counts) {
        colors.extend(std::iter::repeat_n(category.color, count));
    }
    colors.resize(cells - reserved_cells, FREE);
    colors.resize(cells, RESERVED);
    (0..rows)
        .map(|row| {
            (0..columns)
                .map(|column| {
                    let top = colors[row * 2 * columns + column];
                    let bottom = colors[(row * 2 + 1) * columns + column];
                    if top == RESERVED && bottom == RESERVED {
                        swatch(RESERVED, '░', styled)
                    } else if top == bottom {
                        swatch(top, '█', styled)
                    } else if styled {
                        format!(
                            "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m▀\x1b[0m",
                            top.0, top.1, top.2, bottom.0, bottom.1, bottom.2
                        )
                    } else {
                        "▀".into()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}
