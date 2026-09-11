#[cfg(target_os = "linux")]
mod directories;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod metadata;

#[cfg(target_os = "linux")]
pub(super) use linux::{prepare, Entry};

#[cfg(not(target_os = "linux"))]
pub(super) struct Entry;

/// 【插件回收站】【平台边界】尚无安全目录句柄适配的平台明确拒绝，不退回永久删除
/// @param target 已授权普通文件；cancelled 为调用取消标记
/// @returns 不可用错误，源文件保持不变
#[cfg(not(target_os = "linux"))]
pub(super) fn prepare(
    _: &super::target::Target,
    _: &std::sync::atomic::AtomicBool,
) -> anyhow::Result<Entry> {
    anyhow::bail!("safe plugin file trashing is unavailable on this platform")
}

#[cfg(not(target_os = "linux"))]
impl Entry {
    /// 【插件回收站】【禁止降级】不支持的平台不能提交回收站操作
    /// @param target 已授权目标
    /// @returns 固定不可用错误
    pub(super) fn commit(&mut self, _: &super::target::Target) -> anyhow::Result<bool> {
        anyhow::bail!("safe plugin file trashing is unavailable on this platform")
    }
}
