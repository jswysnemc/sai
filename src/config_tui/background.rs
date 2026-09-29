use super::{input::read_key_with_timeout, ui::draw_menu};
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::event::KeyCode;
use std::{io, sync::mpsc, time::Duration};

/// 【配置界面】【后台请求】显示等待状态，在工作线程处理请求，Esc 立即返回。
/// 参数: stdout 为终端，title 为标题，work 为有限超时任务；返回: 结果或取消
pub(super) fn run<T: Send + 'static>(
    stdout: &mut io::Stdout,
    title: &str,
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<Option<Result<T, String>>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(work().map_err(|error| error.to_string()));
    });
    let mut size = None;
    loop {
        let current = crossterm::terminal::size().ok();
        if current != size || size.is_none() {
            draw_menu(
                stdout,
                title,
                &[t("Connecting…", "正在连接…").into()],
                0,
                "Esc / q",
            )?;
            size = current;
        }
        match receiver.try_recv() {
            Ok(result) => return Ok(Some(result)),
            Err(mpsc::TryRecvError::Disconnected) => {
                return Ok(Some(Err(t("Request stopped", "请求已中断").into())))
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        if matches!(
            read_key_with_timeout(Some(Duration::from_millis(50)))?,
            Some(KeyCode::Esc | KeyCode::Char('q'))
        ) {
            return Ok(None);
        }
    }
}
