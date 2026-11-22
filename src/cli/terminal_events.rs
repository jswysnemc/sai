//! 统一终端输入事件层：封装跨平台轮询、Windows ConPTY CSI-u 解码与修饰键规范化。

#[cfg(any(windows, test))]
pub(crate) mod csi_u;
pub(crate) mod normalize;
#[cfg(any(windows, test))]
mod windows_reader;

use crossterm::event::Event;
use std::io;
use std::time::Duration;

pub(crate) use normalize::{is_alt_char, is_ctrl_char};

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
pub(crate) fn ready() -> io::Result<bool> {
    poll(ready_timeout(cfg!(unix)))
}

/// 【终端输入】【超时轮询】等待终端事件到达；Windows 通过原生队列拦截 ConPTY CSI-u。
///
/// 参数:
/// - `timeout`: 最大等待时长
///
/// 返回:
/// - 有可读取事件时返回 true
pub(crate) fn poll(timeout: Duration) -> io::Result<bool> {
    #[cfg(windows)]
    {
        windows_reader::poll(timeout)
    }
    #[cfg(not(windows))]
    {
        crossterm::event::poll(timeout)
    }
}

/// 【终端输入】【事件读取】读取并规范化下一条终端事件，消除大小写与协议编码差异。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 规范化后的终端事件
pub(crate) fn read() -> io::Result<Event> {
    #[cfg(windows)]
    {
        windows_reader::read()
    }
    #[cfg(not(windows))]
    {
        crossterm::event::read().map(normalize::normalize_event)
    }
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
