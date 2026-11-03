//! 内部滚动边界的单次阻挡与后续同向放行。

use crate::render::transcript::ParagraphKey;

/// 【终端】【滚轮目标】区分执行链和独立思考窗口，避免共享边界状态。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum WheelTarget {
    Activity(ParagraphKey),
    Reasoning(ParagraphKey),
}

/// 【终端】【边界交接】保存首次阻挡的目标与方向，后续同向事件允许外层滚动。
#[derive(Default)]
pub(super) struct WheelBoundary {
    blocked: Option<(WheelTarget, isize)>,
}

impl WheelBoundary {
    /// 【终端】【边界放行】内部仍能移动时保留滚轮，到边界先阻挡一次，再放行同向事件。
    /// 参数: `target` 为内部目标，`delta` 为滚轮位移，`moved` 为内部是否移动；返回: 是否允许外层滚动。
    pub(super) fn allows_outer(&mut self, target: WheelTarget, delta: isize, moved: bool) -> bool {
        if moved {
            self.clear();
            return false;
        }
        let boundary = (target, delta.signum());
        if self.blocked == Some(boundary) {
            return true;
        }
        self.blocked = Some(boundary);
        false
    }

    /// 【终端】【指针边界】指针仍在原目标内时保留交接状态，离开或切换目标时清除。
    /// 参数: `target` 为指针当前命中的内部目标；返回: 无。
    pub(super) fn retain_target(&mut self, target: Option<WheelTarget>) {
        if self
            .blocked
            .is_some_and(|(blocked, _)| Some(blocked) != target)
        {
            self.clear();
        }
    }

    /// 【终端】【边界重置】其他输入开始新的交互，清除上一次滚轮交接状态。
    /// 参数: 无；返回: 无。
    pub(super) fn clear(&mut self) {
        self.blocked = None;
    }
}
