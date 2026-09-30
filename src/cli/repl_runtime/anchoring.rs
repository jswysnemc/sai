//! 主屏受管区域的重新锚定：终端高度变化与外部程序写屏后校正原点记账。

use super::viewport::TerminalSize;
use super::ReplRuntime;
use crate::render::terminal_paint::paint_lock;
use anyhow::Result;
use std::io::{self, Write};

impl ReplRuntime {
    /// 仅高度变化时按光标位移修正锚点记账。
    ///
    /// 终端缩放会保持光标可见：变矮把内容上滚（顶部行进入 scrollback），
    /// 变高可能把 scrollback 行拉回屏幕。比较上次绘制的光标行与当前实际
    /// 光标行即可得到内容位移量，同步 origin 与 offscreen 记账。
    ///
    /// 参数:
    /// - `size`: 新终端尺寸
    ///
    /// 返回:
    /// - 是否成功重锚（失败时调用方退回全量重建）
    pub(super) fn reanchor_for_height_change(&mut self, size: TerminalSize) -> bool {
        // 测试环境无真实终端，光标查询会阻塞
        if cfg!(test) {
            return false;
        }
        let Some(expected) = self.last_cursor_row else {
            return false;
        };
        let Ok((_, actual)) = crossterm::cursor::position() else {
            return false;
        };
        let delta = i32::from(expected) - i32::from(actual);
        if delta > 0 {
            // 1. 变矮：内容上移 delta 行；越过 origin 的部分已滚入 scrollback
            let delta = delta.min(i32::from(u16::MAX)) as u16;
            let absorbed = delta.min(self.viewport.origin_row());
            self.viewport.apply_terminal_scroll(delta);
            self.stream.note_scrolled(delta.saturating_sub(absorbed));
        } else if delta < 0 {
            // 2. 变高：scrollback 行被拉回，受管区起点下移且拉回行重新可修补
            let rise = (-delta).min(i32::from(u16::MAX)) as u16;
            self.viewport.shift_origin_down(rise, size);
            self.stream.note_unscrolled(usize::from(rise));
        }
        true
    }

    /// 外部程序写过终端后，从当前光标行重启受管区域。
    ///
    /// 已有输出全部视作 scrollback 保留原样，后续内容从光标处追加。
    pub(super) fn restart_after_external(&mut self) -> Result<()> {
        self.desynced = false;
        // 光标查询要读终端的真实位置，未提交的绘制必须先送达
        self.commit_frame()?;
        let _paint = paint_lock();
        let mut stdout = io::stdout();
        let position = crossterm::cursor::position().unwrap_or((0, 0));
        if position.0 != 0 {
            write!(stdout, "\r\n")?;
            stdout.flush()?;
        }
        let size = TerminalSize::current();
        let origin = crossterm::cursor::position()
            .map(|(_, row)| row)
            .unwrap_or(position.1);
        self.viewport.restart_at(size, origin);
        self.stream.mark_all_offscreen();
        Ok(())
    }
}
