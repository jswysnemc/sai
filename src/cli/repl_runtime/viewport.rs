use crossterm::{cursor, terminal};

/// REPL 终端可用尺寸。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TerminalSize {
    pub(super) cols: u16,
    pub(super) rows: u16,
}

impl TerminalSize {
    /// 读取当前终端尺寸。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 至少为一行一列的终端尺寸
    pub(super) fn current() -> Self {
        let (cols, rows) = terminal::size().unwrap_or((80, 24));
        Self {
            cols: cols.max(1),
            rows: rows.max(1),
        }
    }
}

/// 终端底部 composer 与顶部历史区的边界。
#[derive(Clone, Copy)]
pub(super) struct InlineViewport {
    size: TerminalSize,
    origin_row: u16,
    composer_height: u16,
    history_height: u16,
}

impl InlineViewport {
    /// 创建空 composer 的 inline viewport。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 初始 viewport
    pub(super) fn new() -> Self {
        // 测试环境不向终端查询光标，避免 cursor position 请求阻塞
        if cfg!(test) {
            return Self {
                size: TerminalSize {
                    cols: 80,
                    rows: 24,
                },
                origin_row: 0,
                composer_height: 0,
                history_height: 0,
            };
        }
        Self {
            size: TerminalSize::current(),
            origin_row: cursor::position().map(|(_, row)| row).unwrap_or(0),
            composer_height: 0,
            history_height: 0,
        }
    }

    /// 更新终端尺寸与 composer 高度。
    ///
    /// 参数:
    /// - `size`: 最新终端尺寸
    /// - `composer_height`: composer 所需行数
    /// - `history_rows`: 当前 transcript 的视觉行数
    ///
    /// 返回:
    /// - 历史区边界是否变化
    pub(super) fn update(
        &mut self,
        size: TerminalSize,
        composer_height: u16,
        history_rows: usize,
    ) -> bool {
        self.origin_row = self.origin_row.min(size.rows.saturating_sub(1));
        let next_height = composer_height.min(size.rows);
        let available_rows = size.rows.saturating_sub(self.origin_row);
        let max_history_height = available_rows.saturating_sub(next_height);
        let next_history_height = (history_rows.min(usize::from(max_history_height))) as u16;
        let changed = self.size != size
            || self.composer_height != next_height
            || self.history_height != next_history_height;
        self.size = size;
        self.composer_height = next_height;
        self.history_height = next_history_height;
        changed
    }

    /// 记录完整终端滚动对受管区域原点的影响。
    ///
    /// 参数:
    /// - `rows`: 本次追加触发的终端滚动行数
    pub(super) fn apply_terminal_scroll(&mut self, rows: u16) {
        if rows == 0 {
            return;
        }
        let moved = rows.min(self.origin_row);
        self.origin_row = self.origin_row.saturating_sub(moved);
        let max_history_height = self
            .size
            .rows
            .saturating_sub(self.origin_row)
            .saturating_sub(self.composer_height);
        self.history_height = self
            .history_height
            .saturating_add(moved)
            .min(max_history_height);
    }

    /// 外部程序写入终端后，从指定行重新开始受管区域。
    ///
    /// 参数:
    /// - `size`: 当前终端尺寸
    /// - `row`: 新的受管区域起始行
    pub(super) fn restart_at(&mut self, size: TerminalSize, row: u16) {
        self.size = size;
        self.origin_row = row.min(size.rows.saturating_sub(1));
        self.history_height = 0;
    }

    /// 返回当前终端尺寸。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 当前终端尺寸
    pub(super) fn size(&self) -> TerminalSize {
        self.size
    }

    /// 返回 composer 顶部行号。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 从零开始的 composer 顶部行号
    pub(super) fn composer_top(&self) -> u16 {
        self.origin_row.saturating_add(self.history_height)
    }

    /// 返回可滚动历史区域的高度。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 历史区域行数
    pub(super) fn history_height(&self) -> u16 {
        self.history_height
    }

    /// 返回 Sai 受管区域的起始行。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 从零开始的终端行号
    pub(super) fn origin_row(&self) -> u16 {
        self.origin_row
    }

    /// 返回 composer 当前保留的行数。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - composer 行数
    pub(super) fn composer_height(&self) -> u16 {
        self.composer_height
    }
}

#[cfg(test)]
mod tests {
    use super::{InlineViewport, TerminalSize};

    /// 验证短 transcript 时 composer 紧跟内容尾部而非强制贴底。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn composer_follows_short_history() {
        let mut viewport = InlineViewport::new();

        viewport.update(TerminalSize { cols: 80, rows: 24 }, 4, 7);

        assert_eq!(viewport.history_height(), 7);
        assert_eq!(viewport.composer_top(), 7);
    }

    /// 验证历史超出可视区域后 composer 固定在终端底部。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn composer_pins_to_bottom_for_long_history() {
        let mut viewport = InlineViewport::new();

        viewport.update(TerminalSize { cols: 80, rows: 24 }, 4, 50);

        assert_eq!(viewport.history_height(), 20);
        assert_eq!(viewport.composer_top(), 20);
    }

    /// 验证终端滚动后受管区域原点同步上移，composer 位置保持不变。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn terminal_scroll_moves_origin_without_moving_composer() {
        let mut viewport = InlineViewport::new();
        viewport.origin_row = 5;
        viewport.update(TerminalSize { cols: 80, rows: 24 }, 3, 80);
        let composer_top = viewport.composer_top();

        viewport.apply_terminal_scroll(2);

        assert_eq!(viewport.origin_row(), 3);
        assert_eq!(viewport.composer_top(), composer_top);
    }
}
