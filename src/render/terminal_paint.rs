use std::sync::{Mutex, MutexGuard, OnceLock};

/// 终端绘制帧锁，避免动画线程与主线程交错写入 ANSI 控制序列。
static TERMINAL_PAINT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// 获取一次终端绘制帧的独占锁。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 当前终端绘制锁守卫
pub(crate) fn paint_lock() -> MutexGuard<'static, ()> {
    TERMINAL_PAINT_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 非阻塞获取终端绘制帧锁。
///
/// 信号恢复线程用它等待当前帧写完；锁被占用时返回 `None`，由调用方决定是否继续等待。
///
/// 返回:
/// - 获取成功时的锁守卫
#[cfg(unix)]
pub(crate) fn try_paint_lock() -> Option<MutexGuard<'static, ()>> {
    match TERMINAL_PAINT_LOCK.get_or_init(|| Mutex::new(())).try_lock() {
        Ok(guard) => Some(guard),
        Err(std::sync::TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
        Err(std::sync::TryLockError::WouldBlock) => None,
    }
}
