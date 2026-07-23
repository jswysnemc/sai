use super::command_result_block::render_live_command_output_for_cli;
use super::streaming_replace::{clear_rendered_rows, rendered_visual_rows};
use super::work_status::{format_elapsed, STATUS_PULSE_FRAMES};
use crate::render::tool_view::command_output_buffer::CommandOutputBuffer;
use crate::tools::command::{CommandOutputChunk, CommandOutputStream};
use anyhow::Result;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// 保存普通 CLI 当前前台命令的有限实时输出，并内嵌 working 动效。
pub(crate) struct CliCommandPreview {
    state: Arc<Mutex<PreviewState>>,
    animating: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

struct PreviewState {
    stdout: CommandOutputBuffer,
    stderr: CommandOutputBuffer,
    rendered_rows: usize,
    active: bool,
    started: Instant,
    frame: usize,
}

impl CliCommandPreview {
    /// 创建空的 CLI 命令输出预览。
    pub(crate) fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(PreviewState {
                stdout: CommandOutputBuffer::default(),
                stderr: CommandOutputBuffer::default(),
                rendered_rows: 0,
                active: false,
                started: Instant::now(),
                frame: 0,
            })),
            animating: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }

    /// 是否正在展示前台命令输出预览。
    pub(crate) fn is_active(&self) -> bool {
        self.state.lock().map(|state| state.active).unwrap_or(false)
    }

    /// 开始新的前台命令输出预览。
    ///
    /// 仅重置缓冲与标记；动效在首个输出分块到达后再启动，
    /// 避免在命令块 / 其它工具输出写入期间用相对清屏擦掉终端内容。
    pub(crate) fn begin(&mut self) {
        self.stop_animation();
        if let Ok(mut state) = self.state.lock() {
            state.stdout = CommandOutputBuffer::default();
            state.stderr = CommandOutputBuffer::default();
            state.rendered_rows = 0;
            // 1. 标记会话已开始，但尚未占用终端行
            state.active = true;
            state.started = Instant::now();
            state.frame = 0;
        }
        // 2. 不在此处 start_animation：此时光标仍在命令块区域之外，
        //    后台 clear_rendered_rows 会按相对位置上移并擦除后续正常输出
    }

    /// 追加命令输出分块并重绘摘要（含工作动效行）。
    pub(crate) fn append(&mut self, chunk: &CommandOutputChunk) -> Result<bool> {
        if let Ok(mut state) = self.state.lock() {
            state.active = true;
            let target = match chunk.stream {
                CommandOutputStream::Stdout => &mut state.stdout,
                CommandOutputStream::Stderr => &mut state.stderr,
            };
            target.append(&chunk.bytes, chunk.omitted_bytes);
        }
        if !self.animating.load(Ordering::SeqCst) {
            self.start_animation();
        }
        redraw_preview(&self.state)
    }

    /// 清除当前实时摘要并释放终端行。
    pub(crate) fn clear(&mut self) -> Result<()> {
        self.stop_animation();
        let mut state = self.state.lock().unwrap();
        state.active = false;
        if state.rendered_rows == 0 {
            return Ok(());
        }
        let mut stdout = io::stdout();
        write!(stdout, "{}", clear_rendered_rows(state.rendered_rows))?;
        stdout.flush()?;
        state.rendered_rows = 0;
        Ok(())
    }

    fn start_animation(&mut self) {
        if self.animating.swap(true, Ordering::SeqCst) {
            return;
        }
        let state = Arc::clone(&self.state);
        let running = Arc::clone(&self.animating);
        self.handle = Some(thread::spawn(move || {
            while running.load(Ordering::SeqCst) {
                if let Ok(mut guard) = state.lock() {
                    if !guard.active {
                        break;
                    }
                    guard.frame = guard.frame.wrapping_add(1);
                }
                let _ = redraw_preview(&state);
                thread::sleep(Duration::from_millis(120));
            }
            running.store(false, Ordering::SeqCst);
        }));
    }

    fn stop_animation(&mut self) {
        self.animating.store(false, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    #[cfg(test)]
    pub(super) fn display_texts(&self) -> (String, String) {
        let state = self.state.lock().unwrap();
        (
            state.stdout.display_text().into_owned(),
            state.stderr.display_text().into_owned(),
        )
    }
}

impl Drop for CliCommandPreview {
    fn drop(&mut self) {
        let _ = self.clear();
    }
}

/// 重绘命令输出与内嵌 working 动效行。
fn redraw_preview(state: &Arc<Mutex<PreviewState>>) -> Result<bool> {
    let mut guard = state.lock().unwrap();
    if !guard.active {
        return Ok(false);
    }
    let mut rendered = render_live_command_output_for_cli(
        &guard.stdout.display_text(),
        &guard.stderr.display_text(),
    );
    // 1. 命令输出下方保留 working 动效，避免 WaitSpinner 锚点被 clear 冲掉
    let pulse = STATUS_PULSE_FRAMES[guard.frame % STATUS_PULSE_FRAMES.len()];
    let elapsed = format_elapsed(guard.started.elapsed());
    let status = format!(
        "\x1b[2m\x1b[36m{pulse} {elapsed}\x1b[0m"
    );
    if rendered.trim().is_empty() {
        rendered = status;
    } else {
        if !rendered.ends_with('\n') {
            rendered.push('\n');
        }
        rendered.push_str(&status);
    }
    let mut stdout = io::stdout();
    if guard.rendered_rows > 0 {
        write!(stdout, "{}", clear_rendered_rows(guard.rendered_rows))?;
    }
    let block = format!("{rendered}\n");
    write!(stdout, "{block}")?;
    stdout.flush()?;
    guard.rendered_rows = rendered_visual_rows(&block);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_resets_previous_command_buffers() {
        let mut preview = CliCommandPreview::new();
        {
            let mut state = preview.state.lock().unwrap();
            state.stdout.append(b"first", 0);
            state.stderr.append(b"error", 0);
        }
        preview.begin();
        assert_eq!(preview.display_texts(), (String::new(), String::new()));
        assert!(preview.is_active());
        // begin 不应立刻启动动画线程（否则会在命令块写入期间相对清屏）
        assert!(!preview.animating.load(Ordering::SeqCst));
        preview.clear().unwrap();
    }
}
