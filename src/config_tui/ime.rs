//! 配置界面期间暂时关闭 fcitx，避免 `/`、j/k 被输入法吃掉。
//!
//! 进入配置时若输入法是激活的，先关掉并记住；文本框和外部编辑器里再打开，
//! 离开配置界面时恢复。没有 fcitx 时这些调用会失败并被忽略。

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

/// 进入配置前输入法是否处于激活状态。离开界面时按此恢复。
static RESTORE: AtomicBool = AtomicBool::new(false);

/// 配置会话期间的输入法守卫。
pub(crate) struct ImeGuard;

impl ImeGuard {
    /// 若输入法当前激活，则关闭它并在守卫释放时恢复。
    ///
    /// 返回:
    /// - 绑定到配置会话生命周期的守卫
    pub(crate) fn suspend() -> Self {
        if fcitx_active() {
            RESTORE.store(true, Ordering::Relaxed);
            fcitx("-c");
        }
        Self
    }
}

impl Drop for ImeGuard {
    fn drop(&mut self) {
        if RESTORE.swap(false, Ordering::Relaxed) {
            fcitx("-o");
        }
    }
}

/// 进入文本编辑时，按进入配置前的状态打开输入法。
pub(crate) fn enable_for_edit() {
    if RESTORE.load(Ordering::Relaxed) {
        fcitx("-o");
    }
}

/// 离开文本编辑、回到列表导航时关闭输入法。
pub(crate) fn disable_for_nav() {
    if RESTORE.load(Ordering::Relaxed) {
        fcitx("-c");
    }
}

/// 查询 fcitx5 是否处于激活状态（状态字符 2）。
fn fcitx_active() -> bool {
    let Ok(output) = Command::new("fcitx5-remote").output() else {
        return false;
    };
    output.stdout.first().copied() == Some(b'2')
}

/// 执行 fcitx5-remote，并等待结束，避免留下僵尸进程。
fn fcitx(arg: &str) {
    let _ = Command::new("fcitx5-remote").arg(arg).output();
}
