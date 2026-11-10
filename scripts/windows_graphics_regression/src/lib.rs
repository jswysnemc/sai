//! 直接编译生产探测模块，在独立 Windows 控制台中验证真实输入记录与模式恢复。
#![cfg(test)]
#![allow(dead_code)]

#[path = "../../../src/render/terminal_image/probe.rs"]
mod probe;
#[path = "../../../src/render/terminal_image/probe_response.rs"]
mod probe_response;
#[cfg(windows)]
#[path = "../../../src/render/terminal_image/probe_windows.rs"]
mod probe_windows;
#[cfg(windows)]
#[path = "../../../src/platform/windows_graphics_probe.rs"]
pub(crate) mod windows_graphics_probe;
#[cfg(windows)]
mod platform {
    pub(crate) use crate::windows_graphics_probe;
}

/// 【终端测试】【未知环境】无终端品牌提示时依赖实际查询结果。
/// 参数: 无；返回: 是否已探测到 Kitty 支持。
fn supports_kitty_graphics() -> bool {
    probe::kitty_supported().unwrap_or(false)
}

/// 【终端测试】【未知环境】隔离品牌推断，使用主动查询通道。
/// 参数: 无；返回: false，样本不提供 iTerm2 标识。
fn supports_iterm_inline_image() -> bool {
    false
}

/// 【终端测试】【未知环境】样本不预设 Sixel 能力。
/// 参数: 无；返回: false，测试必须实际识别应答。
fn supports_windows_terminal_sixel() -> bool {
    false
}

#[cfg(all(windows, test))]
mod console_fixture;
#[cfg(all(windows, test))]
mod native_tests;
#[cfg(all(windows, test))]
mod terminal_fixture;
