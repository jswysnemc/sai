//! 点击区域直接读取渲染帧收集到的几何热区，危险操作仍经过固定目标确认。

use super::{model::TaskManager, view::TaskFrame};
use crossterm::event::{KeyCode, MouseEvent};

/// 【后台任务】【控件命中】按已渲染热区映射点击坐标到对应按键，不依赖文本拆词推测。
///
/// 参数:
/// - `manager`: 任务管理状态
/// - `mouse`: 鼠标事件
/// - `frame`: 当前已渲染帧几何
///
/// 返回:
/// - 命中控件时返回对应动作键码
pub(super) fn hit(manager: &TaskManager, mouse: MouseEvent, frame: &TaskFrame) -> Option<KeyCode> {
    if !frame.rect.contains(mouse.column, mouse.row) {
        return None;
    }
    if let Some(button) = frame.buttons.iter().find(|button| {
        button.row == mouse.row && mouse.column >= button.start_col && mouse.column < button.end_col
    }) {
        // 1. 【后台任务】【状态安全】确认与繁忙态下禁止筛选与通道按钮响应
        let bottom = frame
            .rect
            .y
            .saturating_add(frame.rect.height.saturating_sub(1));
        if (manager.confirm.is_some() || manager.busy) && button.row != bottom {
            return None;
        }
        return Some(button.key);
    }
    None
}
