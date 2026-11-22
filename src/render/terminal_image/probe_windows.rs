//! Windows 图形查询使用原生输入记录，支持 Kitty、Sixel 设备属性及 CSI 16 t 单元格像素探测。

use super::probe::{GraphicsCapabilities, QUERY, QUERY_ID, TIMEOUT};
use super::probe_response::{ProbeReply, ProbeResponse};
use crate::platform::windows_graphics_probe::{response_fragment, ConsoleQuery};
use std::io::{self, Write};
use std::time::{Duration, Instant};
use windows_sys::Win32::System::Console::INPUT_RECORD;

const CELL_SIZE_QUERY: &[u8] = b"\x1b[16t";
const CELL_SIZE_TIMEOUT: Duration = Duration::from_millis(80);

/// 【终端图片】【Windows 探测】主动验证 Kitty，失败或超时时使用 DA1 验证 Sixel。
/// 参数: 无；返回: 实际图形能力或系统错误，普通输入始终归还控制台。
pub(super) fn query() -> io::Result<GraphicsCapabilities> {
    query_with_output(&mut io::stdout())
}

/// 【终端图片】【Windows 像素】通过 CSI 16 t 查询终端单元格像素尺寸（ELYTerminal/Winc 经 ConPTY 透传）。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 终端返回的 `(宽像素, 高像素)` 或 None，普通输入始终归还控制台
pub(super) fn query_cell_size() -> io::Result<Option<(usize, usize)>> {
    query_cell_size_with_output(&mut io::stdout())
}

/// 【终端图片】【像素通道】将 CSI 16 t 写入指定输出并解析单元格像素应答。
///
/// 参数:
/// - `output`: 终端输出写入器
///
/// 返回:
/// - 单元格像素尺寸或系统错误
pub(super) fn query_cell_size_with_output(
    output: &mut impl Write,
) -> io::Result<Option<(usize, usize)>> {
    let console = ConsoleQuery::open()?;
    let mut response = ProbeResponse::new(QUERY_ID);
    // 1. 【终端图片】【孤立 ST】清理宿主或 ConPTY 可能残留的 ST，避免污染后续草稿
    response.set_discard_orphan_st(true);
    let result = query_cell_size_inner(&console, &mut response, output);
    let restored = console.restore_input(response.into_input());
    restored?;
    result
}

/// 【终端图片】【查询通道】将协议写入指定输出，原生输入与模式管理由同一实现负责。
/// 参数: output 为终端输出；返回: 图形能力或系统错误，测试可注入终端应答。
pub(super) fn query_with_output(output: &mut impl Write) -> io::Result<GraphicsCapabilities> {
    let console = ConsoleQuery::open()?;
    let mut response = ProbeResponse::new(QUERY_ID);
    // 1. 【终端图片】【查询清理】即使输出或读取失败，也先归还已接收的用户输入
    let result = query_capabilities(&console, &mut response, output);
    let restored = console.restore_input(response.into_input());
    restored?;
    result
}

/// 【终端图片】【协议顺序】Kitty 成功时不额外发送 DA1，避免留下未消费的后续响应。
/// 参数: console 为原生通道，response 为应答解析器，output 为协议输出；返回: 图形能力或系统错误。
fn query_capabilities(
    console: &ConsoleQuery,
    response: &mut ProbeResponse<INPUT_RECORD>,
    output: &mut impl Write,
) -> io::Result<GraphicsCapabilities> {
    let mut capabilities = GraphicsCapabilities {
        kitty: false,
        sixel: Some(false),
        cell_size: None,
    };
    for (request, kitty_query) in [(QUERY, true), (b"\x1b[c".as_slice(), false)] {
        response.finish_query();
        // 1. 【终端图片】【ConPTY 容错】仅在 Kitty APC 查询期间丢弃 ConPTY 吞掉正文后残留的孤立 ST
        response.set_discard_orphan_st(kitty_query);
        output.write_all(request)?;
        output.flush()?;
        let deadline = Instant::now() + TIMEOUT;
        while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
            let Some(record) = console.read(remaining)? else {
                break;
            };
            match response.push_fragment(response_fragment(&record), record) {
                Some(ProbeReply::Kitty(supported)) => {
                    capabilities.kitty = supported;
                    if kitty_query {
                        if supported {
                            return Ok(capabilities);
                        }
                        break;
                    }
                }
                Some(ProbeReply::Sixel(supported)) if !kitty_query => {
                    capabilities.sixel = Some(supported);
                    return Ok(capabilities);
                }
                _ => {}
            }
        }
    }
    Ok(capabilities)
}

/// 【终端图片】【单元格查询】发送 CSI 16 t 并在短时限内等待 `CSI 6;h;w t` 响应。
///
/// 参数:
/// - `console`: 原生控制台查询通道
/// - `response`: 共享应答解析器
/// - `output`: 协议输出流
///
/// 返回:
/// - 单元格像素尺寸或 None
fn query_cell_size_inner(
    console: &ConsoleQuery,
    response: &mut ProbeResponse<INPUT_RECORD>,
    output: &mut impl Write,
) -> io::Result<Option<(usize, usize)>> {
    response.finish_query();
    output.write_all(CELL_SIZE_QUERY)?;
    output.flush()?;
    let deadline = Instant::now() + CELL_SIZE_TIMEOUT;
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        let Some(record) = console.read(remaining)? else {
            break;
        };
        if let Some(ProbeReply::CellSize(width, height)) =
            response.push_fragment(response_fragment(&record), record)
        {
            return Ok(Some((width, height)));
        }
    }
    Ok(None)
}
