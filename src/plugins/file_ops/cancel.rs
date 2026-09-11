use anyhow::{bail, Result};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub(in crate::plugins) struct CancelOnDrop(pub(in crate::plugins) Arc<AtomicBool>);

impl Drop for CancelOnDrop {
    /// 【插件文件】【取消信号】异步调用释放时通知尚未提交的文件工作停止
    /// @returns 无；已经开始的文件系统调用不能保证中断
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

/// 【插件文件】【取消检查】在授权、锁等待、准备和交接边界检查撤销
/// @param cancelled 异步调用的取消标记
/// @returns 调用仍有效时成功
pub(in crate::plugins) fn check_cancelled(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Acquire) {
        bail!("plugin file operation cancelled");
    }
    Ok(())
}
