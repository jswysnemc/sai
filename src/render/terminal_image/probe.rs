//! 启动时查询图形能力；Windows 验证 Kitty 和 Sixel，Unix 为未知终端补充 Kitty 检测。

#[cfg(not(windows))]
use super::probe_response::{ProbeReply, ProbeResponse};
use crossterm::event;
#[cfg(not(windows))]
use crossterm::terminal;
#[cfg(not(windows))]
use std::io::Write;
use std::io::{self, IsTerminal};
use std::sync::OnceLock;
use std::time::Duration;
#[cfg(not(windows))]
use std::time::Instant;

pub(super) const QUERY_ID: u32 = 314159265;
pub(super) const QUERY: &[u8] = b"\x1b_Gi=314159265,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\";
pub(super) const TIMEOUT: Duration = Duration::from_millis(200);
static CAPABILITIES: OnceLock<GraphicsCapabilities> = OnceLock::new();

#[derive(Clone, Copy, Default)]
pub(super) struct GraphicsCapabilities {
    pub(super) kitty: bool,
    pub(super) sixel: Option<bool>,
}

/// 【终端图片】【能力缓存】读取启动查询结果，不触发 I/O。
/// 参数: 无；返回: 查询成功或失败的状态，尚未查询时为 None。
pub(super) fn kitty_supported() -> Option<bool> {
    CAPABILITIES.get().map(|value| value.kitty)
}

/// 【终端图片】【Sixel 缓存】返回原生设备属性查询结果，未查询的平台保留兼容策略。
/// 参数: 无；返回: Sixel 能力应答或 None。
pub(super) fn sixel_supported() -> Option<bool> {
    CAPABILITIES.get().and_then(|value| value.sixel)
}

/// 【终端图片】【启动探测】在历史渲染与输入循环之前查询一次未知终端。
/// 参数: 无；返回: 查询期间收到的普通事件，由调用方依次交回输入循环。
pub(crate) fn initialize_terminal_graphics() -> Vec<event::Event> {
    // 1. 【终端图片】【探测范围】保留 iTerm2 选择，跳过重定向与复用器
    if !cfg!(any(unix, windows))
        || CAPABILITIES.get().is_some()
        || !io::stdin().is_terminal()
        || !io::stdout().is_terminal()
        || std::env::var_os("TMUX").is_some()
        || super::supports_iterm_inline_image()
    {
        return Vec::new();
    }
    // 2. 【终端图片】【Windows 查询】原生记录保留 ESC 和用户输入，不能经过键盘字符转换
    #[cfg(windows)]
    {
        CAPABILITIES.get_or_init(|| {
            super::probe_windows::query().unwrap_or(GraphicsCapabilities {
                kitty: false,
                sixel: Some(false),
            })
        });
        Vec::new()
    }
    #[cfg(not(windows))]
    initialize_unix_graphics()
}

/// 【终端图片】【Unix 查询】已知终端沿用原有协议，其他终端使用有界 Kitty 探测。
/// 参数: 无；返回: 探测期间收到的普通事件。
#[cfg(not(windows))]
fn initialize_unix_graphics() -> Vec<event::Event> {
    if super::supports_kitty_graphics() || super::supports_windows_terminal_sixel() {
        return Vec::new();
    }
    let mut response = ProbeResponse::new(QUERY_ID);
    CAPABILITIES.get_or_init(|| GraphicsCapabilities {
        kitty: query(&mut response).unwrap_or(false),
        sixel: None,
    });
    response.into_input()
}

/// 【终端图片】【查询往返】通过已有事件读取器接收响应，避免竞争读取终端字节。
/// 参数: response 为应答解析及普通输入缓冲；返回: 协议是否可用或终端错误。
#[cfg(not(windows))]
fn query(response: &mut ProbeResponse) -> io::Result<bool> {
    let _raw = RawModeGuard::enable()?;
    let mut output = io::stdout();
    output.write_all(QUERY)?;
    output.flush()?;
    let deadline = Instant::now() + TIMEOUT;
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        if !event::poll(remaining)? {
            break;
        }
        if let Some(ProbeReply::Kitty(supported)) = response.push(event::read()?) {
            return Ok(supported);
        }
    }
    Ok(false)
}

/// 仅恢复本次查询临时启用的 raw mode。
#[cfg(not(windows))]
struct RawModeGuard(bool);

#[cfg(not(windows))]
impl RawModeGuard {
    /// 【终端图片】【输入模式】保存查询前的模式，需要时临时关闭终端回显。
    /// 参数: 无；返回: 自动恢复模式的守卫或终端错误。
    fn enable() -> io::Result<Self> {
        let changed = !terminal::is_raw_mode_enabled()?;
        if changed {
            terminal::enable_raw_mode()?;
        }
        Ok(Self(changed))
    }
}

#[cfg(not(windows))]
impl Drop for RawModeGuard {
    /// 【终端图片】【模式恢复】查询成功、失败或提前返回均恢复原有终端模式。
    /// 参数: 无；返回: 无，恢复失败不覆盖原始查询错误。
    fn drop(&mut self) {
        if self.0 {
            let _ = terminal::disable_raw_mode();
        }
    }
}
