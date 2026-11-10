//! Windows 图形查询使用原生输入记录，Kitty 不可用时再查询 Sixel 设备属性。

use super::probe::{GraphicsCapabilities, QUERY, QUERY_ID, TIMEOUT};
use super::probe_response::{ProbeReply, ProbeResponse};
use crate::platform::windows_graphics_probe::{response_fragment, ConsoleQuery};
use std::io::{self, Write};
use std::time::Instant;
use windows_sys::Win32::System::Console::INPUT_RECORD;

/// 【终端图片】【Windows 探测】主动验证 Kitty，失败或超时时使用 DA1 验证 Sixel。
/// 参数: 无；返回: 实际图形能力或系统错误，普通输入始终归还控制台。
pub(super) fn query() -> io::Result<GraphicsCapabilities> {
    query_with_output(&mut io::stdout())
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
    };
    for (request, kitty_query) in [(QUERY, true), (b"\x1b[c".as_slice(), false)] {
        response.finish_query();
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
