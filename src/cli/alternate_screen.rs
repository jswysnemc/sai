//! 备用屏切换与键盘增强协议的配对管理。
//!
//! Kitty 键盘增强协议（`CSI > flags u` / `CSI < u`）在主屏与备用屏各有一个
//! 独立的栈。输入守卫在主屏压栈、却在备用屏里出栈时，主屏栈上会残留一层；
//! sai 退出后 shell 仍收到 `CSI 99;5u` 这类转义序列，Ctrl+C、Ctrl+L 全部失效。
//! 所有切屏都经过这里：离开当前屏前把本进程压入的层数弹掉，到达新屏后再压回去。

use crossterm::event::KeyboardEnhancementFlags;
use crossterm::queue;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use std::io::{self, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

/// 本进程在当前屏幕上压入的键盘增强层数。
static PUSHED: AtomicUsize = AtomicUsize::new(0);

/// 本进程压入键盘增强协议时使用的标志。
pub(super) const ENHANCEMENT_FLAGS: KeyboardEnhancementFlags =
    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES;

/// 返回压入一层键盘增强协议的序列（`CSI > flags u`）。
///
/// 直接写 ANSI 字节而不走 crossterm 命令：crossterm 在 Windows 上会把这条命令
/// 派发到旧式控制台 API 并报 Unsupported；协议本身只在支持它的终端里启用。
fn push_sequence() -> String {
    format!("\x1b[>{}u", ENHANCEMENT_FLAGS.bits())
}

/// 返回弹出指定层数的序列（`CSI < n u`）；层数为零时为空。
fn pop_sequence(layers: usize) -> String {
    if layers == 0 {
        String::new()
    } else {
        format!("\x1b[<{layers}u")
    }
}

/// 【终端】【键盘增强】记录一次压栈并写出协议序列。
///
/// 参数:
/// - `writer`: 终端输出
///
/// 返回:
/// - 写出结果
pub(super) fn push_enhancement<W: Write>(writer: &mut W) -> io::Result<()> {
    writer.write_all(push_sequence().as_bytes())?;
    writer.flush()?;
    PUSHED.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

/// 【终端】【键盘增强】记录一次出栈并写出协议序列；没有压过时不写。
///
/// 参数:
/// - `writer`: 终端输出
///
/// 返回:
/// - 写出结果
pub(super) fn pop_enhancement<W: Write>(writer: &mut W) -> io::Result<()> {
    let popped = PUSHED
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
            value.checked_sub(1)
        })
        .is_ok();
    if popped {
        writer.write_all(pop_sequence(1).as_bytes())?;
        writer.flush()?;
    }
    Ok(())
}

/// 【终端】【切屏】弹掉当前屏幕上的键盘增强层后切屏，再在新屏幕上压回同样层数。
///
/// 参数:
/// - `writer`: 终端输出
/// - `enter`: true 进入备用屏，false 回到主屏
///
/// 返回:
/// - 写出结果
fn switch_screen<W: Write>(writer: &mut W, enter: bool) -> io::Result<()> {
    // 备用屏界面（配置、会话选择等）可能不经过输入守卫，这里同样装好信号恢复
    super::terminal_signals::install();
    let layers = PUSHED.load(Ordering::SeqCst);
    writer.write_all(pop_sequence(layers).as_bytes())?;
    write_screen_switch(writer, enter)?;
    for _ in 0..layers {
        writer.write_all(push_sequence().as_bytes())?;
    }
    writer.flush()
}

/// 写出切屏命令。
///
/// 测试里直接写 ANSI：Windows CI 没有附着控制台，crossterm 会改走旧式控制台 API 并失败；
/// 运行时仍交给 crossterm，旧式 Windows 控制台照常用其 API 切换缓冲区。
///
/// 参数:
/// - `writer`: 终端输出
/// - `enter`: true 进入备用屏
///
/// 返回:
/// - 写出结果
fn write_screen_switch<W: Write>(writer: &mut W, enter: bool) -> io::Result<()> {
    if cfg!(test) {
        let sequence: &[u8] = if enter {
            b"\x1b[?1049h"
        } else {
            b"\x1b[?1049l"
        };
        return writer.write_all(sequence);
    }
    if enter {
        queue!(writer, EnterAlternateScreen)
    } else {
        queue!(writer, LeaveAlternateScreen)
    }
}

/// 进入备用屏，键盘增强层随之迁移。
///
/// 参数:
/// - `writer`: 终端输出
///
/// 返回:
/// - 写出结果
pub(crate) fn enter_alternate_screen<W: Write>(writer: &mut W) -> io::Result<()> {
    switch_screen(writer, true)
}

/// 回到主屏，键盘增强层随之迁移。
///
/// 参数:
/// - `writer`: 终端输出
///
/// 返回:
/// - 写出结果
pub(crate) fn leave_alternate_screen<W: Write>(writer: &mut W) -> io::Result<()> {
    switch_screen(writer, false)
}

/// 【终端】【退出兜底】弹掉本进程仍压着的全部键盘增强层。
///
/// 只弹自己压入的层数，不影响 shell 自身压入的协议状态。
///
/// 参数:
/// - `writer`: 终端输出
///
/// 返回:
/// - 写出结果
pub(super) fn pop_all_enhancement<W: Write>(writer: &mut W) -> io::Result<()> {
    let layers = PUSHED.swap(0, Ordering::SeqCst);
    writer.write_all(pop_sequence(layers).as_bytes())?;
    writer.flush()
}

/// 返回弹掉本进程全部键盘增强层的序列并清零计数，不写任何输出。
///
/// 信号恢复路径自己决定怎么写（绕开 stdout 锁的原始写入），这里只负责生成字节。
///
/// 返回:
/// - `CSI < n u`；没有压栈时为空
pub(super) fn take_pop_all_sequence() -> String {
    pop_sequence(PUSHED.swap(0, Ordering::SeqCst))
}

#[cfg(test)]
pub(super) fn pushed_layers() -> usize {
    PUSHED.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 串行化使用全局计数的测试。
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// 模拟终端：主屏与备用屏各有独立的键盘增强栈。
    #[derive(Default)]
    struct KittyStacks {
        alternate: bool,
        main: usize,
        alt: usize,
    }

    impl KittyStacks {
        /// 按顺序应用输出中的切屏与压栈、出栈序列。
        fn feed(&mut self, bytes: &[u8]) {
            let text = String::from_utf8_lossy(bytes);
            let mut rest = text.as_ref();
            while let Some(at) = rest.find('\x1b') {
                rest = &rest[at..];
                if let Some(tail) = rest.strip_prefix("\x1b[?1049h") {
                    self.alternate = true;
                    rest = tail;
                } else if let Some(tail) = rest.strip_prefix("\x1b[?1049l") {
                    self.alternate = false;
                    rest = tail;
                } else if rest.starts_with("\x1b[>") && rest.contains('u') {
                    *self.stack() += 1;
                    rest = &rest[rest.find('u').unwrap() + 1..];
                } else if let Some(tail) = rest.strip_prefix("\x1b[<") {
                    let end = tail.find('u').unwrap();
                    let count = tail[..end].parse::<usize>().unwrap_or(1);
                    let stack = self.stack();
                    *stack = stack.saturating_sub(count);
                    rest = &tail[end + 1..];
                } else {
                    rest = &rest[1..];
                }
            }
        }

        /// 当前屏幕的栈。
        fn stack(&mut self) -> &mut usize {
            if self.alternate {
                &mut self.alt
            } else {
                &mut self.main
            }
        }
    }

    /// 验证输入守卫跨切屏压栈、出栈后，退出时两块屏幕的栈都回到零。
    #[test]
    fn screen_switches_keep_both_stacks_balanced() {
        let _guard = LOCK.lock().unwrap();
        PUSHED.store(0, Ordering::SeqCst);
        let mut output = Vec::new();
        // 主屏压栈 → 进全屏 → 在备用屏出栈再压栈（下一轮输入） → 回主屏 → 出栈
        push_enhancement(&mut output).unwrap();
        enter_alternate_screen(&mut output).unwrap();
        pop_enhancement(&mut output).unwrap();
        push_enhancement(&mut output).unwrap();
        leave_alternate_screen(&mut output).unwrap();
        pop_enhancement(&mut output).unwrap();
        let mut stacks = KittyStacks::default();
        stacks.feed(&output);
        assert_eq!((stacks.main, stacks.alt), (0, 0));
        assert_eq!(pushed_layers(), 0);
    }

    /// 验证在备用屏里退出时，兜底出栈清掉备用屏的层，主屏也不残留。
    #[test]
    fn exiting_inside_the_alternate_screen_leaves_no_layers() {
        let _guard = LOCK.lock().unwrap();
        PUSHED.store(0, Ordering::SeqCst);
        let mut output = Vec::new();
        push_enhancement(&mut output).unwrap();
        enter_alternate_screen(&mut output).unwrap();
        pop_all_enhancement(&mut output).unwrap();
        leave_alternate_screen(&mut output).unwrap();
        let mut stacks = KittyStacks::default();
        stacks.feed(&output);
        assert_eq!((stacks.main, stacks.alt), (0, 0));
    }

    /// 验证未压栈时出栈不写任何序列，不会弹掉 shell 自己的层。
    #[test]
    fn popping_without_push_writes_nothing() {
        let _guard = LOCK.lock().unwrap();
        PUSHED.store(0, Ordering::SeqCst);
        let mut output = Vec::new();
        pop_enhancement(&mut output).unwrap();
        pop_all_enhancement(&mut output).unwrap();
        assert!(output.is_empty());
    }
}
