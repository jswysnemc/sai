//! 【终端】【鼠标捕获】嵌套面板共享捕获生命周期，最后一层退出时恢复终端。

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use std::io;
use std::sync::Mutex;

static USERS: Mutex<usize> = Mutex::new(0);

pub(super) struct MouseCapture;

impl MouseCapture {
    /// 【终端】【鼠标捕获】进入一层交互界面；无参数，返回守卫或终端错误。
    pub(super) fn acquire() -> io::Result<Self> {
        let mut users = USERS.lock().unwrap_or_else(|error| error.into_inner());
        if *users == 0 && !cfg!(test) {
            crossterm::execute!(io::stdout(), EnableMouseCapture)?;
        }
        *users += 1;
        Ok(Self)
    }
}

impl Drop for MouseCapture {
    /// 【终端】【鼠标恢复】释放本层捕获，保留外层面板的鼠标功能；无参数和返回值。
    fn drop(&mut self) {
        let mut users = USERS.lock().unwrap_or_else(|error| error.into_inner());
        *users = users.saturating_sub(1);
        if *users == 0 && !cfg!(test) {
            let _ = crossterm::execute!(io::stdout(), DisableMouseCapture);
        }
    }
}
