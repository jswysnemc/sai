//! 终端输入就绪检测，屏蔽 Unix 轮询后端的零超时差异。

use std::io;
use std::time::Duration;

/// 【终端输入】【就绪等待】Unix 普通轮询需要正数等待才会读取输入，Windows 保留零等待。
/// 参数: unix 为是否使用 Unix 后端；返回: 有界的就绪检测等待时间。
const fn ready_timeout(unix: bool) -> Duration {
    if unix {
        Duration::from_millis(1)
    } else {
        Duration::ZERO
    }
}

/// 【终端输入】【就绪检测】短时检测已到达事件，避免零超时导致生成期间无法接收滚轮和按键。
/// 参数: 无；返回: 是否有可读取事件，终端读取错误向调用者返回。
pub(super) fn ready() -> io::Result<bool> {
    crossterm::event::poll(ready_timeout(cfg!(unix)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端测试】【平台等待】Unix 等待有上限，Windows 不改变原有检测策略。
    /// 参数: 无；返回: 无。
    #[test]
    fn ready_poll_is_bounded_and_keeps_windows_nonblocking() {
        assert_eq!(ready_timeout(true), Duration::from_millis(1));
        assert_eq!(ready_timeout(false), Duration::ZERO);
    }
}
