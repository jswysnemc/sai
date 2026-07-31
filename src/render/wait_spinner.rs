use crate::render::activity_animation::{
    activity_frame_count, render_activity_detail, render_activity_text, ACTIVITY_FRAME_INTERVAL,
};
use crate::render::content_indent::align_to_guide_column;
use crate::render::work_status::format_elapsed;
use anyhow::Result;
use crossterm::cursor::{self, MoveTo};
use crossterm::execute;
use crossterm::terminal::{self, Clear, ClearType};
use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Instant;

pub(crate) struct WaitSpinner {
    state: Arc<Mutex<WaitSpinnerState>>,
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

struct WaitSpinnerState {
    phase: String,
    sub_phase: Option<String>,
    start: Instant,
    anchor_row: u16,
    lines_rendered: u16,
    /// 启动时预留的行数；运行中新增的副状态不得超出该预留
    reserved_lines: u16,
}

impl WaitSpinner {
    /// 【终端】【等待状态】判断当前终端是否适合显示状态动画。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 标准输出连接终端时返回 true
    pub(crate) fn supported() -> bool {
        io::stdout().is_terminal()
    }

    /// 【终端】【等待状态】启动文字扫光动画，并使用指定起点计时。
    ///
    /// 参数:
    /// - `phase`: 初始状态文字
    /// - `sub_phase`: 初始详情文字，空值表示隐藏
    /// - `started_at`: 本轮计时起点
    ///
    /// 返回:
    /// - 等待状态动画控制器
    pub(crate) fn start_with_clock(
        phase: String,
        sub_phase: Option<String>,
        started_at: Instant,
    ) -> Self {
        let reserved_lines = spinner_line_count(sub_phase.as_deref());
        let anchor_row = reserve_spinner_rows(reserved_lines);
        let state = Arc::new(Mutex::new(WaitSpinnerState {
            phase,
            sub_phase,
            start: started_at,
            anchor_row,
            lines_rendered: 0,
            reserved_lines,
        }));
        let running = Arc::new(AtomicBool::new(true));
        render_initial_spinner_frame(&state);
        let thread_state = Arc::clone(&state);
        let thread_running = Arc::clone(&running);
        let handle = thread::spawn(move || run_spinner_loop(thread_state, thread_running));
        Self {
            state,
            running,
            handle: Some(handle),
        }
    }

    /// 【终端】【等待状态】更新主状态文字且不重置累计计时。
    ///
    /// 参数:
    /// - `phase`: Waiting、Thinking、Working 等新状态文字
    ///
    /// 返回:
    /// - 无
    pub(crate) fn set_phase(&self, phase: impl Into<String>) {
        let phase = phase.into();
        if let Ok(mut state) = self.state.lock() {
            if state.phase != phase {
                state.phase = phase;
            }
        }
    }

    /// 【终端】【等待状态】更新状态详情文字。
    ///
    /// 参数:
    /// - `sub_phase`: 详情文字，空值表示隐藏
    ///
    /// 返回:
    /// - 无
    pub(crate) fn set_sub_phase(&self, sub_phase: Option<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.sub_phase = sub_phase.filter(|value| !value.trim().is_empty());
        }
    }

    /// 【终端】【等待状态】停止动画并清理已经渲染的终端行。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 清理终端行是否成功
    pub(crate) fn stop(&mut self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        let (anchor_row, lines) = self
            .state
            .lock()
            .map(|state| (state.anchor_row, state.lines_rendered))
            .unwrap_or((0, 0));
        clear_spinner_lines(anchor_row, lines)
    }
}

impl Drop for WaitSpinner {
    /// 【终端】【等待状态】释放动画时恢复其占用行。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无；析构阶段忽略终端清理错误
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// 【终端】【等待状态】按固定节拍刷新文字扫光帧。
///
/// 参数:
/// - `state`: 等待动画共享状态
/// - `running`: 动画运行标记
///
/// 返回:
/// - 无
fn run_spinner_loop(state: Arc<Mutex<WaitSpinnerState>>, running: Arc<AtomicBool>) {
    let mut frame = 1usize;
    while running.load(Ordering::SeqCst) {
        let (output, anchor_row, prev_lines, lines, frame_count) = match state.lock() {
            Ok(mut guard) => {
                let prev = guard.lines_rendered;
                let (output, lines) = render_frame(frame, &guard);
                guard.lines_rendered = lines;
                (
                    output,
                    guard.anchor_row,
                    prev,
                    lines,
                    activity_frame_count(&guard.phase),
                )
            }
            Err(_) => (String::new(), 0, 0, 0, 1),
        };
        if !output.is_empty() {
            let _ = write_spinner_lines(&output, anchor_row, prev_lines, lines);
        }
        thread::sleep(ACTIVITY_FRAME_INTERVAL);
        frame = (frame + 1) % frame_count.max(1);
    }
}

/// 【终端】【等待状态】同步渲染等待动画首帧。
///
/// 参数:
/// - `state`: 等待动画共享状态
///
/// 返回:
/// - 无
fn render_initial_spinner_frame(state: &Arc<Mutex<WaitSpinnerState>>) {
    let (output, anchor_row, prev_lines, lines) = match state.lock() {
        Ok(mut guard) => {
            let prev = guard.lines_rendered;
            let (output, lines) = render_frame(0, &guard);
            guard.lines_rendered = lines;
            (output, guard.anchor_row, prev, lines)
        }
        Err(_) => (String::new(), 0, 0, 0),
    };
    if !output.is_empty() {
        let _ = write_spinner_lines(&output, anchor_row, prev_lines, lines);
    }
}

/// 【终端】【等待状态】渲染单帧状态文字与详情。
///
/// 参数:
/// - `frame`: 当前动画帧序号
/// - `state`: 等待动画状态
///
/// 返回:
/// - ANSI 文本与占用行数
fn render_frame(frame: usize, state: &WaitSpinnerState) -> (String, u16) {
    let elapsed = render_activity_detail(&format_elapsed(state.start.elapsed()));
    let phase = render_activity_text(&state.phase, frame);
    let main_line = align_to_guide_column(&format!("{phase} {elapsed}"));
    match &state.sub_phase {
        Some(sub_phase) if !sub_phase.trim().is_empty() => {
            if state.reserved_lines >= 2 {
                let sub_line = format!("  {}", render_activity_detail(sub_phase));
                (format!("{main_line}\n{sub_line}"), 2)
            } else {
                (
                    format!("{main_line} {}", render_activity_detail(sub_phase)),
                    1,
                )
            }
        }
        _ => (main_line, 1),
    }
}

/// 【终端】【等待状态】计算等待动画需要占用的终端行数。
///
/// 参数:
/// - `sub_phase`: 等待动画详情文字
///
/// 返回:
/// - 等待动画渲染行数
fn spinner_line_count(sub_phase: Option<&str>) -> u16 {
    match sub_phase {
        Some(value) if !value.trim().is_empty() => 2,
        _ => 1,
    }
}

/// 【终端】【等待状态】为动画预留终端空间并返回锚点行。
///
/// 参数:
/// - `lines`: 动画需要渲染的行数
///
/// 返回:
/// - 动画锚点行
fn reserve_spinner_rows(lines: u16) -> u16 {
    let row = cursor::position().map(|(_, row)| row).unwrap_or(0);
    let rows = terminal::size().map(|(_, rows)| rows.max(1)).unwrap_or(24);
    let overflow = spinner_row_overflow(row, rows, lines.max(1));
    if overflow > 0 {
        let mut stdout = io::stdout();
        for _ in 0..overflow {
            let _ = writeln!(stdout);
        }
        let _ = stdout.flush();
    }
    row.saturating_sub(overflow)
}

/// 【终端】【等待状态】计算终端底部需要滚动的行数。
///
/// 参数:
/// - `row`: 当前光标行
/// - `rows`: 终端总行数
/// - `lines`: 动画行数
///
/// 返回:
/// - 需要滚动的行数
fn spinner_row_overflow(row: u16, rows: u16, lines: u16) -> u16 {
    row.saturating_add(lines).saturating_sub(rows)
}

/// 【终端】【等待状态】覆盖动画占用的终端行。
///
/// 参数:
/// - `output`: 当前帧 ANSI 文本
/// - `anchor_row`: 动画锚点行
/// - `prev_lines`: 上一帧占用行数
/// - `lines`: 当前帧占用行数
///
/// 返回:
/// - 终端写入是否成功
fn write_spinner_lines(output: &str, anchor_row: u16, prev_lines: u16, lines: u16) -> Result<()> {
    let mut stdout = io::stdout();
    let rows_to_clear = prev_lines.max(lines).max(1);
    for row_offset in 0..rows_to_clear {
        execute!(
            stdout,
            MoveTo(0, anchor_row.saturating_add(row_offset)),
            Clear(ClearType::CurrentLine)
        )?;
    }
    for (index, line) in output.lines().enumerate() {
        execute!(stdout, MoveTo(0, anchor_row.saturating_add(index as u16)))?;
        write!(stdout, "{line}")?;
    }
    stdout.flush()?;
    Ok(())
}

/// 【终端】【等待状态】清除动画曾经占用的终端行。
///
/// 参数:
/// - `anchor_row`: 动画锚点行
/// - `lines`: 需要清除的行数
///
/// 返回:
/// - 终端清理是否成功
fn clear_spinner_lines(anchor_row: u16, lines: u16) -> Result<()> {
    let mut stdout = io::stdout();
    for row_offset in 0..lines {
        execute!(
            stdout,
            MoveTo(0, anchor_row.saturating_add(row_offset)),
            Clear(ClearType::CurrentLine)
        )?;
    }
    execute!(stdout, MoveTo(0, anchor_row))?;
    stdout.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::activity_animation::strip_ansi_for_test;

    /// 【终端】【等待状态】构造测试用动画状态。
    ///
    /// 参数:
    /// - `phase`: 状态文字
    /// - `sub_phase`: 可选详情文字
    ///
    /// 返回:
    /// - 测试用动画状态
    fn make_state(phase: &str, sub_phase: Option<&str>) -> WaitSpinnerState {
        WaitSpinnerState {
            phase: phase.to_string(),
            sub_phase: sub_phase.map(str::to_string),
            start: Instant::now(),
            anchor_row: 0,
            lines_rendered: 0,
            reserved_lines: spinner_line_count(sub_phase),
        }
    }

    /// 【终端】【等待状态测试】验证后出现的详情不会占用未预留行。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn late_sub_phase_stays_within_reserved_single_line() {
        let mut state = make_state("Waiting", None);
        state.sub_phase = Some("model gpt".to_string());

        let (frame, lines) = render_frame(0, &state);

        assert_eq!(lines, 1);
        assert!(!frame.contains('\n'));
        assert!(frame.contains("model gpt"));
    }

    /// 【终端】【等待状态测试】验证状态文字使用 Codex 风格流光且不使用移动点。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn render_frame_uses_white_shimmer_without_moving_dots() {
        let state = make_state("Thinking", None);

        let (first, lines) = render_frame(0, &state);
        let (second, _) = render_frame(1, &state);
        let plain = strip_ansi_for_test(&first);

        assert!(plain.contains("Thinking"));
        assert!(plain.contains("0s"));
        assert!(!plain.contains('·'));
        assert_ne!(first, second);
        assert_eq!(lines, 1);
    }

    /// 【终端】【等待状态测试】验证预留详情时渲染两行。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn render_frame_with_sub_phase_produces_two_lines() {
        let state = make_state("Working", Some("round 1"));

        let (frame, lines) = render_frame(0, &state);
        let plain = strip_ansi_for_test(&frame);

        assert!(plain.contains("Working"));
        assert!(plain.contains("round 1"));
        assert_eq!(lines, 2);
    }

    /// 【终端】【等待状态测试】验证终端底部溢出行数计算。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn spinner_row_overflow_reserves_bottom_space() {
        assert_eq!(spinner_row_overflow(22, 24, 2), 0);
        assert_eq!(spinner_row_overflow(23, 24, 2), 1);
        assert_eq!(spinner_row_overflow(23, 24, 1), 0);
    }

    /// 【终端】【等待状态测试】验证详情文字计入预留行数。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn spinner_line_count_includes_non_empty_detail() {
        assert_eq!(spinner_line_count(Some("model: test")), 2);
        assert_eq!(spinner_line_count(Some("  ")), 1);
        assert_eq!(spinner_line_count(None), 1);
    }
}
