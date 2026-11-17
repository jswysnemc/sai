//! 后台任务响应式布局：宽屏采用左右双栏（队列 + 详情/日志），窄屏自适应上下堆叠。

/// 列表与详情区域的最小双栏阈值
const MIN_SPLIT_WIDTH: usize = 86;
/// 启用左右分栏的最小正文高度
const MIN_SPLIT_HEIGHT: usize = 9;
/// 堆叠模式下默认详情区行数：详情标题 + 3 行详情字段
const DEFAULT_DETAIL_ROWS: usize = 4;
/// 堆叠模式下窄屏命令展开时的详情区最大行数
const MAX_STACKED_EXPANDED_DETAIL_ROWS: usize = 8;
/// 堆叠模式下尾部日志最少占 2 行（标题 + 首行输出）
const MIN_STACKED_OUTPUT_ROWS: usize = 2;

/// 【后台任务】【列表布局】区分宽屏双栏与窄屏上下堆叠的行宽和高度预算。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SplitLayout {
    Split {
        left_w: usize,
        right_w: usize,
        task_rows: usize,
        detail_rows: usize,
        tail_rows: usize,
    },
    Stacked {
        task_rows: usize,
        tail_rows: usize,
    },
}

impl SplitLayout {
    /// 【后台任务】【详情列宽】在渲染详情行前预先计算详情区域的实际可用列宽。
    ///
    /// 参数:
    /// - `inner`: 去除外框左右边距后的可用列数
    /// - `body_rows`: 面板正文总行数
    ///
    /// 返回:
    /// - 双栏模式下的右栏列宽或堆叠模式下的整行净宽
    pub(super) fn detail_width(inner: usize, body_rows: usize) -> usize {
        if inner >= MIN_SPLIT_WIDTH && body_rows >= MIN_SPLIT_HEIGHT {
            let left_w = ((inner * 42) / 100).clamp(32, 48);
            inner.saturating_sub(left_w + 3)
        } else {
            inner
        }
    }

    /// 【后台任务】【尺寸决策】按面板净宽与正文高度计算双栏或堆叠分区。
    ///
    /// 参数:
    /// - `inner`: 去除外框左右边距后的可用列数
    /// - `body_rows`: 去除外框上下边框后的正文总行数
    /// - `total`: 当前任务总数（用于窄屏贴合任务数）
    /// - `confirming`: 是否处于确认模式（确认卡片固定占用右侧或下半区）
    ///
    /// 返回:
    /// - 稳定的面板布局参数
    #[cfg(test)]
    pub(super) fn compute(inner: usize, body_rows: usize, total: usize, confirming: bool) -> Self {
        Self::compute_with_detail(inner, body_rows, total, confirming, 0).0
    }

    /// 【后台任务】【展开尺寸决策】综合命令展开后的视觉行数分配详情与尾部日志高度预算。
    ///
    /// 参数:
    /// - `inner`: 去除外框左右边距后的可用列数
    /// - `body_rows`: 去除外框上下边框后的正文总行数
    /// - `total`: 当前任务总数
    /// - `confirming`: 是否处于确认模式
    /// - `expanded_cmd_rows`: 命令展开时的折行后总行数（未展开时传 0）
    ///
    /// 返回:
    /// - `(面板布局参数, 详情区分配行数)`
    pub(super) fn compute_with_detail(
        inner: usize,
        body_rows: usize,
        total: usize,
        confirming: bool,
        expanded_cmd_rows: usize,
    ) -> (Self, usize) {
        if inner >= MIN_SPLIT_WIDTH && body_rows >= MIN_SPLIT_HEIGHT {
            // 1. 【后台任务】【宽屏分栏】左侧任务队列占约 42%，右侧按命令展开高度动态让渡尾部空间
            let left_w = ((inner * 42) / 100).clamp(32, 48);
            let right_w = inner.saturating_sub(left_w + 3);
            let content_rows = body_rows.saturating_sub(2);
            let detail_rows = if confirming {
                content_rows.min(6)
            } else if expanded_cmd_rows > 0 {
                let max_detail = (content_rows * 3 / 5)
                    .max(DEFAULT_DETAIL_ROWS)
                    .min(content_rows);
                3usize
                    .saturating_add(expanded_cmd_rows)
                    .max(DEFAULT_DETAIL_ROWS)
                    .min(max_detail)
            } else {
                DEFAULT_DETAIL_ROWS
            };
            let tail_rows = if confirming {
                0
            } else {
                content_rows.saturating_sub(detail_rows + 1)
            };
            return (
                Self::Split {
                    left_w,
                    right_w,
                    task_rows: content_rows.max(1),
                    detail_rows,
                    tail_rows,
                },
                detail_rows,
            );
        }

        // 2. 【后台任务】【窄屏堆叠】展开命令时最多扩展至 8 行详情，剩余弹性高度分给任务与日志尾页
        let detail_rows = if confirming || expanded_cmd_rows == 0 {
            DEFAULT_DETAIL_ROWS
        } else {
            let max_stacked = body_rows
                .saturating_sub(3)
                .max(DEFAULT_DETAIL_ROWS)
                .min(MAX_STACKED_EXPANDED_DETAIL_ROWS);
            3usize
                .saturating_add(expanded_cmd_rows)
                .max(DEFAULT_DETAIL_ROWS)
                .min(max_stacked)
        };
        let fixed_rows = 2usize.saturating_add(detail_rows);
        let flexible = body_rows.saturating_sub(fixed_rows);
        let cap = (flexible * 2 / 3).max(flexible.min(3)).max(1);
        let tasks = total.clamp(1, cap);
        let tail = flexible.saturating_sub(tasks);
        let layout = if tail < MIN_STACKED_OUTPUT_ROWS || confirming {
            Self::Stacked {
                task_rows: flexible.max(1),
                tail_rows: 0,
            }
        } else {
            Self::Stacked {
                task_rows: tasks,
                tail_rows: tail,
            }
        };
        (layout, detail_rows)
    }
}
