//! 【终端】【鼠标捕获】嵌套面板共享捕获生命周期，最后一层退出时恢复终端。

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use std::io;
use std::sync::Mutex;

static USERS: Mutex<usize> = Mutex::new(0);

pub(crate) struct MouseCapture;

impl MouseCapture {
    /// 【终端】【鼠标捕获】进入一层交互界面；无参数，返回守卫或终端错误。
    pub(crate) fn acquire() -> io::Result<Self> {
        let mut users = USERS.lock().unwrap_or_else(|error| error.into_inner());
        if *users == 0 && !cfg!(test) {
            crossterm::execute!(io::stdout(), EnableMouseCapture)?;
        }
        *users += 1;
        Ok(Self)
    }

    /// 【终端】【外部进程挂起】调用外部编辑器期间临时关闭鼠标捕获，返回后按引用计数自动重开。
    /// 参数: action 为外部进程执行闭包；返回: 闭包执行结果。
    pub(crate) fn suspend_for<T, F: FnOnce() -> T>(action: F) -> T {
        let active = {
            let users = USERS.lock().unwrap_or_else(|error| error.into_inner());
            *users > 0
        };
        if active && !cfg!(test) {
            let _ = crossterm::execute!(io::stdout(), DisableMouseCapture);
        }
        let result = action();
        let still_active = {
            let users = USERS.lock().unwrap_or_else(|error| error.into_inner());
            *users > 0
        };
        if still_active && !cfg!(test) {
            let _ = crossterm::execute!(io::stdout(), EnableMouseCapture);
        }
        result
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
