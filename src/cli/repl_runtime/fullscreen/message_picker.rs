//! 用户消息选择面板的焦点与滚动窗口。

#[derive(Default)]
pub(super) struct MessagePicker {
    pub(super) active: bool,
    pub(super) selected: usize,
    pub(super) start: usize,
}

impl MessagePicker {
    /// 【消息导航】【展开列表】默认选中当前消息，并将选中项放入可见窗口。
    /// 参数: current 为当前消息，count 为消息总数，visible 为可见条数；返回: 是否打开。
    pub(super) fn open(&mut self, current: Option<usize>, count: usize, visible: usize) -> bool {
        if count == 0 {
            return false;
        }
        self.active = true;
        self.selected = current.unwrap_or(0).min(count - 1);
        self.start = self.selected.saturating_sub(visible / 2);
        self.sync(count, visible);
        true
    }

    /// 【消息导航】【关闭列表】只关闭面板，不改变会话位置。
    /// 参数: 无；返回: 无。
    pub(super) fn close(&mut self) {
        self.active = false;
    }

    /// 【消息导航】【窗口同步】新增消息或缩放后夹紧焦点，确保选中项始终可见。
    /// 参数: count 为消息总数，visible 为可见条数；返回: 无。
    pub(super) fn sync(&mut self, count: usize, visible: usize) {
        if count == 0 {
            self.close();
            return;
        }
        let visible = visible.max(1);
        self.selected = self.selected.min(count - 1);
        self.start = self.start.min(count.saturating_sub(visible));
        if self.selected < self.start {
            self.start = self.selected;
        } else if self.selected >= self.start + visible {
            self.start = self.selected + 1 - visible;
        }
    }

    /// 【消息导航】【移动焦点】键盘和滚轮共用列表导航，不滚动下方会话。
    /// 参数: delta 为移动条数，count 为消息总数，visible 为窗口条数；返回: 焦点是否变化。
    pub(super) fn move_by(&mut self, delta: isize, count: usize, visible: usize) -> bool {
        self.select(self.selected.saturating_add_signed(delta), count, visible)
    }

    /// 【消息导航】【指定焦点】选择消息并同步滚动窗口。
    /// 参数: index 为消息序号，count 为总数，visible 为窗口条数；返回: 焦点是否变化。
    pub(super) fn select(&mut self, index: usize, count: usize, visible: usize) -> bool {
        let previous = self.selected;
        self.selected = index.min(count.saturating_sub(1));
        self.sync(count, visible);
        self.selected != previous
    }
}
