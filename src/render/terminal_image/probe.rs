//! 启动时主动查询未知终端的 Kitty 图形能力，渲染阶段只读取缓存。

use super::probe_response::ProbeResponse;
use crossterm::{event, terminal};
use std::io::{self, IsTerminal, Write};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub(super) const QUERY_ID: u32 = 314159265;
const QUERY: &[u8] = b"\x1b_Gi=314159265,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\";
const TIMEOUT: Duration = Duration::from_millis(200);
static KITTY_SUPPORTED: OnceLock<bool> = OnceLock::new();

/// 【终端图片】【能力缓存】读取启动查询结果，不触发 I/O。
/// 参数: 无；返回: 是否已收到匹配查询编号的成功应答。
pub(super) fn kitty_supported() -> bool {
    KITTY_SUPPORTED.get().copied().unwrap_or(false)
}

/// 【终端图片】【启动探测】在历史渲染与输入循环之前查询一次未知终端。
/// 参数: 无；返回: 查询期间收到的普通事件，由调用方依次交回输入循环。
pub(crate) fn initialize_terminal_graphics() -> Vec<event::Event> {
    // 1. 【终端图片】【探测范围】已知协议沿用原有选择，跳过重定向、复用器和旧式控制台
    if !cfg!(unix)
        || KITTY_SUPPORTED.get().is_some()
        || !io::stdin().is_terminal()
        || !io::stdout().is_terminal()
        || std::env::var_os("TMUX").is_some()
        || super::supports_kitty_graphics()
        || super::supports_iterm_inline_image()
        || super::supports_windows_terminal_sixel()
    {
        return Vec::new();
    }
    let mut response = ProbeResponse::new(QUERY_ID);
    // 2. 【终端图片】【有界查询】q 动作只验证直接传输，不存储或显示测试图片
    KITTY_SUPPORTED.get_or_init(|| query(&mut response).unwrap_or(false));
    response.into_input()
}

/// 【终端图片】【查询往返】通过已有事件读取器接收响应，避免竞争读取终端字节。
/// 参数: response 为应答解析及普通输入缓冲；返回: 协议是否可用或终端错误。
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
        if let Some(supported) = response.push(event::read()?) {
            return Ok(supported);
        }
    }
    Ok(false)
}

/// 仅恢复本次查询临时启用的 raw mode。
struct RawModeGuard(bool);

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

impl Drop for RawModeGuard {
    /// 【终端图片】【模式恢复】查询成功、失败或提前返回均恢复原有终端模式。
    /// 参数: 无；返回: 无，恢复失败不覆盖原始查询错误。
    fn drop(&mut self) {
        if self.0 {
            let _ = terminal::disable_raw_mode();
        }
    }
}
