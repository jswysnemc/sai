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
