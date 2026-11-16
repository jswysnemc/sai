//! 后台任务响应式布局：宽屏采用左右双栏（队列 + 详情/日志），窄屏自适应上下堆叠。

/// 列表与详情区域的最小双栏阈值
const MIN_SPLIT_WIDTH: usize = 86;
/// 启用左右分栏的最小正文高度
const MIN_SPLIT_HEIGHT: usize = 9;
/// 堆叠模式下固定区行数：筛选栏 + 分隔线 + 详情标题 + 3 行详情
const STACKED_FIXED_ROWS: usize = 6;
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
    /// 【后台任务】【尺寸决策】按面板净宽与正文高度计算双栏或堆叠分区。
    ///
    /// 参数:
    /// - `inner`: 去除外框左右边距后的可用列数
    /// - `body_rows`: 去除筛选栏与分隔线后的可变区域（含总正文行数）
    /// - `total`: 当前任务总数（用于窄屏贴合任务数）
    /// - `confirming`: 是否处于确认模式（确认卡片固定占用右侧或下半区）
    ///
    /// 返回:
    /// - 稳定的面板布局参数
    pub(super) fn compute(inner: usize, body_rows: usize, total: usize, confirming: bool) -> Self {
        if inner >= MIN_SPLIT_WIDTH && body_rows >= MIN_SPLIT_HEIGHT {
            // 1. 【后台任务】【宽屏分栏】左侧任务队列占约 42%，中间留 3 列分隔，右侧详情与日志占剩余空间
            let left_w = ((inner * 42) / 100).clamp(32, 48);
            let right_w = inner.saturating_sub(left_w + 3);
            let content_rows = body_rows.saturating_sub(2);
            let detail_rows = if confirming { content_rows.min(6) } else { 4 };
            let tail_rows = if confirming {
                0
            } else {
                content_rows.saturating_sub(detail_rows + 1)
            };
            return Self::Split {
                left_w,
                right_w,
                task_rows: content_rows.max(1),
                detail_rows,
                tail_rows,
            };
        }

        // 2. 【后台任务】【窄屏堆叠】任务区按总数自适应并封顶 2/3，剩余空间全部给日志尾页
        let flexible = body_rows.saturating_sub(STACKED_FIXED_ROWS);
        let cap = (flexible * 2 / 3).max(flexible.min(3)).max(1);
        let tasks = total.clamp(1, cap);
        let tail = flexible.saturating_sub(tasks);
        if tail < MIN_STACKED_OUTPUT_ROWS || confirming {
            Self::Stacked {
                task_rows: flexible.max(1),
                tail_rows: 0,
            }
        } else {
            Self::Stacked {
                task_rows: tasks,
                tail_rows: tail,
            }
        }
    }
}
