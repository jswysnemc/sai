use anyhow::{Context, Result};
use rusqlite::ffi;
use std::ptr::NonNull;

/// 【SQLite 内存】【独占分配】反序列化接管前自动回收 SQLite 分配的内存
pub(crate) struct Allocation(NonNull<u8>);

impl Allocation {
    /// 【SQLite 内存】【完整初始化】分块复制来源并清零剩余空间，检查函数可提前中止
    /// @param source 原镜像；capacity 为已校验容量；check 为字节计量与取消检查
    /// @returns 已初始化的独占缓冲，任何失败均释放内存
    pub(crate) fn new(
        source: &[u8],
        capacity: usize,
        mut check: impl FnMut(usize) -> Result<()>,
    ) -> Result<Self> {
        // 1. 【SQLite 内存】【分配安全】上层校验容量和来源长度，分配失败不产生可使用指针
        let pointer = NonNull::new(unsafe { ffi::sqlite3_malloc64(capacity as u64) }.cast::<u8>())
            .context("allocate SQLite snapshot")?;
        let allocation = Self(pointer);
        for offset in (0..capacity).step_by(32768) {
            let count = (capacity - offset).min(32768);
            check(count)?;
            let copied = source.len().saturating_sub(offset).min(count);
            // 2. 【SQLite 内存】【复制安全】新分配与来源不重叠；复制和清零均限制在容量内
            unsafe {
                if copied > 0 {
                    std::ptr::copy_nonoverlapping(
                        source.as_ptr().add(offset),
                        pointer.as_ptr().add(offset),
                        copied,
                    );
                }
                std::ptr::write_bytes(pointer.as_ptr().add(offset + copied), 0, count - copied);
            }
        }
        Ok(allocation)
    }

    /// 【SQLite 内存】【所有权移交】消耗守卫，调用方必须立即交给带 FREEONCLOSE 的反序列化
    /// @returns 唯一分配指针，移交后本守卫不再析构
    pub(crate) fn into_raw(self) -> *mut u8 {
        let pointer = self.0.as_ptr();
        std::mem::forget(self);
        pointer
    }
}

impl Drop for Allocation {
    /// 【SQLite 内存】【失败回收】释放尚未交给 SQLite 的内存
    /// @returns 无；指针来自 sqlite3_malloc64，且仍由本守卫独占
    fn drop(&mut self) {
        // 1. 【SQLite 内存】【释放安全】只有未移交的唯一守卫执行释放，不与 SQLite 重复回收
        unsafe { ffi::sqlite3_free(self.0.as_ptr().cast()) };
    }
}
