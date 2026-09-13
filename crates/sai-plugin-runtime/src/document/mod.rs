mod options;
mod render;

use anyhow::{ensure, Result};
pub(crate) use options::{Mode, Options};
pub(crate) use render::render;

pub(crate) const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const MAX_RENDERED_BYTES: usize = 16 * 1024 * 1024;
const WORKSPACE_OVERHEAD: usize = 64 * 1024;

/// 【文档转换】【工作预留】在调用解码和 HTML 库之前检查输入并计量共享工作额度
/// @param input_bytes 原始字节数；mode 为转换方式；output_bytes 为结构化输出上限
/// @returns 本次工作需要持有的预留字节数，不包含已经计费的输入缓冲
pub(crate) fn working_bytes(input_bytes: usize, mode: Mode, output_bytes: usize) -> Result<usize> {
    ensure!(
        input_bytes <= MAX_INPUT_BYTES,
        "document input exceeds size limit"
    );
    // 1. 【文档转换】【替换解码】每个无效原始字节最多展开成三个 UTF-8 字节
    let decoded = input_bytes * 3;
    // 2. 【文档转换】【库工作区】DOM 预留是调度估算，不是第三方分配器的硬内存限制
    let conversion = match mode {
        Mode::Raw => 0,
        Mode::HtmlText | Mode::HtmlMarkdown => input_bytes * 5 + MAX_RENDERED_BYTES,
    };
    Ok(decoded + conversion + output_bytes + WORKSPACE_OVERHEAD)
}
