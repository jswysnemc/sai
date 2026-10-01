use anyhow::{bail, Result};

/// 【模型接口】【完成校验】检查协议结束信号，防止部分输出被保存为完整回复。
/// @param protocol 为协议名称；completed 表示已解析明确结束事件
/// @returns 完成时成功，否则返回可保留部分输出的传输错误
pub(super) fn require_completion(protocol: &str, completed: bool) -> Result<()> {
    if !completed {
        bail!("{protocol} stream ended before the upstream signalled completion");
    }
    Ok(())
}
