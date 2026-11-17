use super::{
    input::{read_event_with_timeout, ConfigEvent},
    mouse::ClickTracker,
    ui::draw_menu_interactive,
};
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
    let mut hitbox = None;
    let mut clicks = ClickTracker::default();
    loop {
        let current = crossterm::terminal::size().ok();
        if current != size || size.is_none() {
            hitbox = Some(draw_menu_interactive(
                stdout,
                title,
                &[t("Connecting…", "正在连接…").into()],
                &[],
                0,
                &super::mouse::ListMouseState::default(),
                &super::theme::help_line(&[("Esc", t("cancel", "取消"))]),
                "",
            )?);
            size = current;
        }
        match receiver.try_recv() {
            Ok(result) => return Ok(Some(result)),
            Err(mpsc::TryRecvError::Disconnected) => {
                return Ok(Some(Err(t("Request stopped", "请求已中断").into())))
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        match read_event_with_timeout(Some(Duration::from_millis(50)))? {
            None => {}
            Some(ConfigEvent::Resize) => size = None,
            Some(ConfigEvent::Key(key))
                if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) =>
            {
                return Ok(None);
            }
            Some(ConfigEvent::Mouse(mouse)) => {
                let target = hitbox
                    .as_ref()
                    .and_then(|hb| hb.footer.hit(mouse.column, mouse.row));
                if clicks.update(mouse, target).is_some() {
                    return Ok(None);
                }
            }
            Some(ConfigEvent::Key(_)) | Some(ConfigEvent::Paste(_)) => {}
        }
    }
}
