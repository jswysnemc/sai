use crate::render::work_status::STATUS_PULSE_FRAMES;
use anyhow::Result;
use crossterm::cursor::{self, MoveTo};
use crossterm::execute;
use crossterm::terminal::{self, Clear, ClearType};
use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const WIDTH: usize = 8;
const TRAIL_LEN: usize = 6;
const HOLD_END: usize = 9;
const HOLD_START: usize = 30;
const INTERVAL: Duration = Duration::from_millis(80);
const MIN_FADE_ALPHA: f64 = 0.12;
const ACCENT_RGB: Rgb = Rgb(133, 153, 0);
const BASE_RGB: (f64, f64, f64) = (0.522, 0.600, 0.0);
const ACTIVE_DOT: &str = "■";
const INACTIVE_DOT: &str = "⬝";
const DEFAULT_FACE: &str = "(◕‿◕)";
const BLINK_FACE: &str = "(-‿-)";
const SLEEPY_FACE: &str = "(◡‿◡)";
const GLANCE_FACES: [&str; 3] = ["(◐‿◐)", "(◑‿◑)", "(◔‿◔)"];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpinnerStyle {
    /// 旧版扫描表情动效；测试与兼容保留。
    #[allow(dead_code)]
    Scanner,
    /// 末行工作状态动效（Braille + 文案 + 耗时）
    Braille,
}

#[derive(Clone, Copy)]
struct ScannerState {
    active_position: usize,
    is_holding: bool,
    hold_progress: usize,
    hold_total: usize,
    movement_progress: usize,
    movement_total: usize,
    is_moving_forward: bool,
}

pub(crate) struct WaitSpinner {
    state: Arc<Mutex<WaitSpinnerState>>,
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

struct WaitSpinnerState {
    phase: String,
    sub_phase: Option<String>,
    start: Instant,
    seed: u64,
    anchor_row: u16,
    lines_rendered: u16,
    style: SpinnerStyle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Rgb(u8, u8, u8);

impl WaitSpinner {
    /// 判断当前终端是否适合显示等待动画。
    ///
    /// 返回:
    /// - 是否可以显示等待动画
    pub(crate) fn supported() -> bool {
        io::stdout().is_terminal()
    }

    /// 启动等待响应动画。
    ///
    /// 参数:
    /// - `phase`: 初始状态文本
    /// - `style`: 动画样式
    /// - `sub_phase`: 初始子状态文本，空值表示隐藏
    ///
    /// 返回:
    /// - 等待动画控制器
    #[allow(dead_code)]
    pub(crate) fn start(phase: String, style: SpinnerStyle, sub_phase: Option<String>) -> Self {
        Self::start_with_clock(phase, style, sub_phase, Instant::now())
    }

    /// 启动等待响应动画，并使用指定起点计时。
    ///
    /// 参数:
    /// - `phase`: 初始状态文本
    /// - `style`: 动画样式
    /// - `sub_phase`: 初始子状态文本，空值表示隐藏
    /// - `started_at`: 本轮计时起点
    ///
    /// 返回:
    /// - 等待动画控制器
    pub(crate) fn start_with_clock(
        phase: String,
        style: SpinnerStyle,
        sub_phase: Option<String>,
        started_at: Instant,
    ) -> Self {
        let anchor_row = reserve_spinner_rows(spinner_line_count(sub_phase.as_deref()));
        let state = Arc::new(Mutex::new(WaitSpinnerState {
            phase,
            sub_phase,
            start: started_at,
            seed: spinner_seed(),
            anchor_row,
            lines_rendered: 0,
            style,
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

    /// 更新末行动效的主状态文案；不重置本轮累计计时。
    ///
    /// 参数:
    /// - `phase`: 新的状态文案
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

    /// 更新末行动效的副状态文案。
    ///
    /// 参数:
    /// - `sub_phase`: 副状态，空值表示隐藏
    ///
    /// 返回:
    /// - 无
    pub(crate) fn set_sub_phase(&self, sub_phase: Option<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.sub_phase = sub_phase.filter(|value| !value.trim().is_empty());
        }
    }

    /// 停止等待动画并清理已渲染行。
    ///
    /// 返回:
    /// - 停止是否成功
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
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn run_spinner_loop(state: Arc<Mutex<WaitSpinnerState>>, running: Arc<AtomicBool>) {
    let mut frame = 1usize;
    let mut cycle = 0usize;
    while running.load(Ordering::SeqCst) {
        let (output, anchor_row, prev_lines, lines, total) = match state.lock() {
            Ok(mut guard) => {
                let prev = guard.lines_rendered;
                let total = total_frames_for_style(guard.style);
                let absolute_frame = cycle * total + frame;
                let (output, lines) = render_frame(absolute_frame, &guard);
                guard.lines_rendered = lines;
                (output, guard.anchor_row, prev, lines, total)
            }
            Err(_) => (String::new(), 0, 0, 0, 1),
        };
        if !output.is_empty() {
            let _ = write_spinner_lines(&output, anchor_row, prev_lines, lines);
        }
        thread::sleep(INTERVAL);
        frame += 1;
        if frame >= total.max(1) {
            frame = 0;
            cycle += 1;
        }
    }
}

/// 同步渲染等待动画首帧。
///
/// 参数:
/// - `state`: 等待动画共享状态
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

fn render_frame(frame: usize, state: &WaitSpinnerState) -> (String, u16) {
    let elapsed_text = format_elapsed(state.start.elapsed());
    let (spinner_prefix, phase, elapsed) = match state.style {
        SpinnerStyle::Scanner => {
            let prefix = render_scanner_prefix(frame, state.seed);
            (
                prefix,
                paint_bold_rgb(&state.phase, ACCENT_RGB),
                paint_faint(&format!(" ({elapsed_text})")),
            )
        }
        SpinnerStyle::Braille => (
            paint_secondary(STATUS_PULSE_FRAMES[frame % STATUS_PULSE_FRAMES.len()]),
            paint_secondary(&state.phase),
            paint_secondary(&elapsed_text),
        ),
    };
    let main_line = match state.style {
        SpinnerStyle::Scanner => format!("{spinner_prefix} {phase}{elapsed}"),
        // 与 TUI 工作动效一致：点跳动 + 耗时
        SpinnerStyle::Braille => format!("{spinner_prefix}{elapsed}"),
    };
    match &state.sub_phase {
        Some(sub_phase) if !sub_phase.trim().is_empty() => {
            let sub_line = format!("  {}", paint_secondary(sub_phase));
            (format!("{main_line}\n{sub_line}"), 2)
        }
        _ => (main_line, 1),
    }
}

/// 计算等待动画需要占用的终端行数。
///
/// 参数:
/// - `sub_phase`: 等待动画详情行
///
/// 返回:
/// - 等待动画渲染行数
fn spinner_line_count(sub_phase: Option<&str>) -> u16 {
    match sub_phase {
        Some(value) if !value.trim().is_empty() => 2,
        _ => 1,
    }
}

/// 为等待动画预留终端空间并返回锚点行。
///
/// 参数:
/// - `lines`: 等待动画需要渲染的行数
///
/// 返回:
/// - 等待动画锚点行
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

/// 计算等待动画在终端底部需要滚动的行数。
///
/// 参数:
/// - `row`: 当前光标行
/// - `rows`: 终端总行数
/// - `lines`: 等待动画行数
///
/// 返回:
/// - 需要滚动的行数
fn spinner_row_overflow(row: u16, rows: u16, lines: u16) -> u16 {
    row.saturating_add(lines).saturating_sub(rows)
}

/// 渲染旧版表情扫描等待动画前缀。
///
/// 参数:
/// - `frame`: 当前全局帧序号
/// - `seed`: 表情切换种子
///
/// 返回:
/// - 带 ANSI 样式的表情和扫描点
fn render_scanner_prefix(frame: usize, seed: u64) -> String {
    let scanner = scanner_state(frame % total_frames_scanner());
    let mut output = String::new();
    output.push_str(&paint_rgb(face_for_frame(frame, seed), ACCENT_RGB));
    output.push(' ');
    for index in 0..WIDTH {
        output.push_str(&render_cell(index, scanner));
    }
    output
}

fn render_cell(char_index: usize, state: ScannerState) -> String {
    let fade = fade_factor(state);
    match color_index(char_index, state) {
        Some(index) if index < TRAIL_LEN => paint_active_dot(index),
        _ => paint_inactive_dot(fade),
    }
}

fn paint_active_dot(index: usize) -> String {
    paint_rgb(ACTIVE_DOT, trail_rgb(index))
}

fn paint_inactive_dot(fade: f64) -> String {
    paint_rgb(INACTIVE_DOT, inactive_rgb(fade))
}

fn total_frames_scanner() -> usize {
    WIDTH + HOLD_END + (WIDTH - 1) + HOLD_START
}

fn total_frames_for_style(style: SpinnerStyle) -> usize {
    match style {
        SpinnerStyle::Scanner => total_frames_scanner(),
        SpinnerStyle::Braille => STATUS_PULSE_FRAMES.len(),
    }
}

fn scanner_state(mut frame: usize) -> ScannerState {
    if frame < WIDTH {
        return ScannerState {
            active_position: frame,
            is_holding: false,
            hold_progress: 0,
            hold_total: 0,
            movement_progress: frame,
            movement_total: WIDTH,
            is_moving_forward: true,
        };
    }
    frame -= WIDTH;
    if frame < HOLD_END {
        return ScannerState {
            active_position: WIDTH - 1,
            is_holding: true,
            hold_progress: frame,
            hold_total: HOLD_END,
            movement_progress: 0,
            movement_total: 0,
            is_moving_forward: true,
        };
    }
    frame -= HOLD_END;
    if frame < WIDTH - 1 {
        return ScannerState {
            active_position: WIDTH - 2 - frame,
            is_holding: false,
            hold_progress: 0,
            hold_total: 0,
            movement_progress: frame,
            movement_total: WIDTH - 1,
            is_moving_forward: false,
        };
    }
    frame -= WIDTH - 1;
    ScannerState {
        active_position: 0,
        is_holding: true,
        hold_progress: frame,
        hold_total: HOLD_START,
        movement_progress: 0,
        movement_total: 0,
        is_moving_forward: false,
    }
}

fn color_index(char_index: usize, state: ScannerState) -> Option<usize> {
    let distance = if state.is_moving_forward {
        state.active_position as isize - char_index as isize
    } else {
        char_index as isize - state.active_position as isize
    };
    if state.is_holding {
        return usize::try_from(distance)
            .ok()
            .map(|distance| distance + state.hold_progress);
    }
    if distance == 0 {
        return Some(0);
    }
    if distance > 0 && distance < TRAIL_LEN as isize {
        return usize::try_from(distance).ok();
    }
    None
}

fn fade_factor(state: ScannerState) -> f64 {
    if state.is_holding && state.hold_total > 0 {
        let progress = (state.hold_progress as f64 / state.hold_total as f64).min(1.0);
        (1.0 - progress * (1.0 - MIN_FADE_ALPHA)).max(MIN_FADE_ALPHA)
    } else if !state.is_holding && state.movement_total > 0 {
        let denominator = state.movement_total.saturating_sub(1).max(1);
        let progress = (state.movement_progress as f64 / denominator as f64).min(1.0);
        MIN_FADE_ALPHA + progress * (1.0 - MIN_FADE_ALPHA)
    } else {
        1.0
    }
}

/// 根据帧序号选择表情。
///
/// 参数:
/// - `frame`: 全局帧序号
/// - `seed`: 表情切换种子
///
/// 返回:
/// - 表情文本
fn face_for_frame(frame: usize, seed: u64) -> &'static str {
    let mut blink_position = 0usize;
    let mut blink_segment = 0usize;
    while blink_position <= frame {
        let hash = face_hash(blink_segment * 7 + seed as usize);
        let gap = 25 + hash % 25;
        if frame >= blink_position + gap && frame < blink_position + gap + 3 {
            return BLINK_FACE;
        }
        blink_position += gap + 3;
        blink_segment += 1;
    }
    if frame > 200 {
        return SLEEPY_FACE;
    }
    let mut position = 0usize;
    let mut segment = 0usize;
    while position <= frame {
        let hash = face_hash(segment + seed as usize);
        let segment_len = 25 + hash % 50;
        if frame < position + segment_len {
            let index = (hash / 50) % (GLANCE_FACES.len() + 1);
            if index < GLANCE_FACES.len() {
                return GLANCE_FACES[index];
            }
            return DEFAULT_FACE;
        }
        position += segment_len;
        segment += 1;
    }
    DEFAULT_FACE
}

/// 生成表情选择用哈希值。
///
/// 参数:
/// - `value`: 输入数字
///
/// 返回:
/// - 哈希值
fn face_hash(value: usize) -> usize {
    let mut value = value as u64;
    value = ((value >> 16) ^ value).wrapping_mul(0x45d9f3b);
    value = ((value >> 16) ^ value).wrapping_mul(0x45d9f3b);
    ((value >> 16) ^ value) as usize
}

/// 生成当前进程的动画种子。
///
/// 返回:
/// - 动画种子
fn spinner_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64 & 0x7fff_ffff)
        .unwrap_or(0)
}

/// 计算尾迹颜色。
///
/// 参数:
/// - `index`: 尾迹索引
///
/// 返回:
/// - RGB 颜色
fn trail_rgb(index: usize) -> Rgb {
    let (mut r, mut g, mut b) = BASE_RGB;
    let alpha = match index {
        0 => 1.0,
        1 => {
            r = (r * 1.15).min(1.0);
            g = (g * 1.15).min(1.0);
            b = (b * 1.15).min(1.0);
            0.9
        }
        _ => 0.65_f64.powi(index.saturating_sub(1) as i32),
    };
    Rgb(
        clamp8(r * alpha * 255.0),
        clamp8(g * alpha * 255.0),
        clamp8(b * alpha * 255.0),
    )
}

/// 计算非活动点颜色。
///
/// 参数:
/// - `factor`: 淡入淡出系数
///
/// 返回:
/// - RGB 颜色
fn inactive_rgb(factor: f64) -> Rgb {
    let (r, g, b) = BASE_RGB;
    let alpha = 0.6 * factor;
    Rgb(
        clamp8(r * alpha * 255.0),
        clamp8(g * alpha * 255.0),
        clamp8(b * alpha * 255.0),
    )
}

/// 限制颜色通道范围。
///
/// 参数:
/// - `value`: 原始浮点颜色值
///
/// 返回:
/// - 8 位颜色通道
fn clamp8(value: f64) -> u8 {
    value.clamp(0.0, 255.0) as u8
}

/// 渲染真彩前景色文本。
///
/// 参数:
/// - `text`: 原始文本
/// - `rgb`: 文本颜色
///
/// 返回:
/// - 带 ANSI 样式的文本
fn paint_rgb(text: &str, rgb: Rgb) -> String {
    format!("\x1b[38;2;{};{};{}m{text}\x1b[0m", rgb.0, rgb.1, rgb.2)
}

/// 渲染加粗真彩前景色文本。
///
/// 参数:
/// - `text`: 原始文本
/// - `rgb`: 文本颜色
///
/// 返回:
/// - 带 ANSI 样式的文本
fn paint_bold_rgb(text: &str, rgb: Rgb) -> String {
    format!(
        "\x1b[1m\x1b[38;2;{};{};{}m{text}\x1b[0m",
        rgb.0, rgb.1, rgb.2
    )
}

/// 渲染弱化文本。
///
/// 参数:
/// - `text`: 原始文本
///
/// 返回:
/// - 带 ANSI 样式的文本
fn paint_faint(text: &str) -> String {
    format!("\x1b[2m{text}\x1b[0m")
}

/// 格式化末行动效耗时。
///
/// 参数:
/// - `elapsed`: 已用时长
///
/// 返回:
/// - 人类可读耗时
fn format_elapsed(elapsed: Duration) -> String {
    let total_secs = elapsed.as_secs();
    if total_secs < 60 {
        let tenths = elapsed.as_millis() / 100;
        format!("{}.{}s", tenths / 10, tenths % 10)
    } else {
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        format!("{mins}m{secs:02}s")
    }
}

fn paint_secondary(text: &str) -> String {
    format!("\x1b[2m\x1b[36m{text}\x1b[0m")
}

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
    let rendered_lines = output.lines().collect::<Vec<_>>();
    for (index, line) in rendered_lines.iter().enumerate() {
        execute!(stdout, MoveTo(0, anchor_row.saturating_add(index as u16)))?;
        write!(stdout, "{line}")?;
    }
    stdout.flush()?;
    Ok(())
}

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

    fn make_state(phase: &str, sub_phase: Option<&str>, style: SpinnerStyle) -> WaitSpinnerState {
        WaitSpinnerState {
            phase: phase.to_string(),
            sub_phase: sub_phase.map(str::to_string),
            start: Instant::now(),
            seed: 0,
            anchor_row: 0,
            lines_rendered: 0,
            style,
        }
    }

    #[test]
    fn render_frame_scanner_has_phase() {
        let state = make_state("思考", None, SpinnerStyle::Scanner);

        let (frame, lines) = render_frame(0, &state);

        assert!(frame.contains("思考"));
        assert!(frame.contains("‿"));
        assert_eq!(lines, 1);
    }

    #[test]
    fn render_frame_with_sub_phase_produces_two_lines() {
        let state = make_state("工具运行中", Some("第 1 轮：诊断中"), SpinnerStyle::Scanner);

        let (frame, lines) = render_frame(0, &state);

        assert!(frame.contains("工具运行中"));
        assert!(frame.contains("第 1 轮"));
        assert_eq!(lines, 2);
    }

    #[test]
    fn spinner_row_overflow_reserves_bottom_space() {
        assert_eq!(spinner_row_overflow(22, 24, 2), 0);
        assert_eq!(spinner_row_overflow(23, 24, 2), 1);
        assert_eq!(spinner_row_overflow(23, 24, 1), 0);
    }

    #[test]
    fn spinner_line_count_includes_non_empty_detail() {
        assert_eq!(spinner_line_count(Some("model: test")), 2);
        assert_eq!(spinner_line_count(Some("  ")), 1);
        assert_eq!(spinner_line_count(None), 1);
    }

    #[test]
    fn pulse_frames_loop_over_pattern() {
        let state = make_state("thinking", None, SpinnerStyle::Braille);

        let (first, _) = render_frame(0, &state);
        let (second, _) = render_frame(STATUS_PULSE_FRAMES.len(), &state);

        assert_eq!(first, second);
        assert!(first.contains(STATUS_PULSE_FRAMES[0]));
    }
}
