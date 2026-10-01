//! 被外部信号终止时恢复终端。
//!
//! `kill`、关闭终端窗口、`timeout` 等发来的 SIGTERM / SIGHUP / SIGQUIT 默认直接结束进程，
//! Drop 与 panic 钩子都不会执行：raw mode、备用屏、鼠标捕获与键盘增强协议全部残留，
//! shell 里 Ctrl+C、Ctrl+L 失效。这里在进入终端界面时安装一次信号监听，收到后先恢复终端再退出。
//!
//! SIGINT 不在这里处理：raw mode 下 Ctrl+C 是普通按键；执行 `!` 命令等 cooked 模式期间
//! SIGINT 交给前台子进程，接管它会改变现有中断行为。SIGKILL 无法捕获，只能在 shell 里执行 `reset`。

/// 恢复终端最多允许耗时；超过即强制退出，终端已关闭或不再读取时不会卡住。
#[cfg(unix)]
const RESTORE_DEADLINE: std::time::Duration = std::time::Duration::from_millis(500);

/// 【终端】【信号恢复】安装一次终止信号监听；重复调用无副作用。
///
/// 返回:
/// - 无；安装失败时保持系统默认行为
#[cfg(unix)]
pub(crate) fn install() {
    use signal_hook::consts::{SIGHUP, SIGQUIT, SIGTERM};
    use std::sync::OnceLock;
    static INSTALLED: OnceLock<()> = OnceLock::new();
    INSTALLED.get_or_init(|| {
        let Ok(mut signals) = signal_hook::iterator::Signals::new([SIGTERM, SIGHUP, SIGQUIT]) else {
            return;
        };
        // 独立线程等待信号：不依赖 tokio 运行时，也不在信号处理函数里做非异步安全的事
        let _ = std::thread::Builder::new()
            .name("sai-terminal-signals".into())
            .spawn(move || {
                if let Some(signal) = signals.forever().next() {
                    exit_for_signal(signal);
                }
            });
    });
}

/// 非 Unix 平台没有这些信号。
#[cfg(not(unix))]
pub(crate) fn install() {}

/// 【终端】【信号恢复】恢复终端后立即结束进程，任何一步失败都不能阻止退出。
///
/// 1. 看门狗线程到期后直接 `_exit`：终端已关闭（SIGHUP）或写入被阻塞时不会卡住
/// 2. 恢复序列经原始 `write(2)` 写出，不取 stdout 锁，写失败直接忽略，绝不 panic
/// 3. 用 `_exit` 而不是 `process::exit`：后者会刷新 stdout，主线程若正持锁就会死锁
///
/// 参数:
/// - `signal`: 收到的信号编号
///
/// 返回:
/// - 不返回
#[cfg(unix)]
fn exit_for_signal(signal: i32) -> ! {
    let code = 128 + signal;
    // 1. 看门狗兜底
    let _ = std::thread::Builder::new()
        .name("sai-terminal-signal-deadline".into())
        .spawn(move || {
            std::thread::sleep(RESTORE_DEADLINE);
            // SAFETY: _exit 只结束进程，不执行任何用户态清理
            unsafe { libc::_exit(code) }
        });
    // 2. 等当前整帧写完，避免恢复序列插进半截转义序列；拿不到锁也继续
    let _paint = wait_for_paint_lock(std::time::Duration::from_millis(150));
    let sequence = super::terminal_restore::restore_sequence();
    write_raw(sequence.as_bytes());
    write_raw(b"\r\n");
    let _ = crossterm::terminal::disable_raw_mode();
    // 3. 立即结束
    // SAFETY: 同上
    unsafe { libc::_exit(code) }
}

/// 在期限内尝试获取绘制锁。
///
/// 参数:
/// - `limit`: 最长等待时间
///
/// 返回:
/// - 锁守卫；超时为空
#[cfg(unix)]
fn wait_for_paint_lock(
    limit: std::time::Duration,
) -> Option<std::sync::MutexGuard<'static, ()>> {
    let deadline = std::time::Instant::now() + limit;
    loop {
        if let Some(guard) = crate::render::terminal_paint::try_paint_lock() {
            return Some(guard);
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// 以 `write(2)` 直接写 stdout，忽略全部错误。
///
/// 参数:
/// - `bytes`: 待写字节
///
/// 返回:
/// - 无
#[cfg(unix)]
fn write_raw(mut bytes: &[u8]) {
    while !bytes.is_empty() {
        // SAFETY: 指针与长度来自同一个有效切片
        let written = unsafe { libc::write(libc::STDOUT_FILENO, bytes.as_ptr().cast(), bytes.len()) };
        if written <= 0 {
            return;
        }
        bytes = &bytes[written as usize..];
    }
}
