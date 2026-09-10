use anyhow::{bail, Result};
use sai_plugin_runtime::host::{
    BuiltinSound, NotificationDelivery, NotificationRequest, NotificationSound,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 【通知投递】【执行控制】平台调用和文件读取共用不可由 Lua 修改的时限与取消标志。
#[derive(Clone)]
pub(crate) struct DeliveryControl {
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
}

impl DeliveryControl {
    /// 【通知投递】【开始】创建仅适用于本次请求的控制状态。
    /// @param timeout 总等待时长
    /// @returns 独立投递控制
    pub(crate) fn new(timeout: Duration) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + timeout,
        }
    }

    /// 【通知投递】【取消】通知阻塞工作线程停止后续动作。
    /// @returns 无
    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// 【通知投递】【检查】在读取、创建平台资源和等待期间检查有效性。
    /// @returns 未取消且未超时时成功
    pub(crate) fn check(&self) -> Result<()> {
        if self.cancelled.load(Ordering::Acquire) {
            bail!("notification delivery was cancelled");
        }
        if Instant::now() >= self.deadline {
            bail!("notification delivery timed out");
        }
        Ok(())
    }
}

/// 【通知投递】【平台边界】只接受已授权的请求与内存音频，不自行选择文件路径。
pub(crate) trait NotificationBackend: Send + Sync {
    /// 【通知投递】【平台执行】完成指定通道，在取消或超时时停止未完成工作。
    /// @param request 已验证请求；audio 为本地音频快照；control 为调用控制
    /// @returns 全部请求通道成功后的结果
    fn deliver(
        &self,
        request: &NotificationRequest,
        audio: Option<&[u8]>,
        control: &DeliveryControl,
    ) -> Result<NotificationDelivery>;
}

pub(crate) struct SystemNotificationBackend;

impl NotificationBackend for SystemNotificationBackend {
    /// 【通知投递】【系统适配】顺序执行固定桌面程序和受控播放，不创建后台业务任务。
    /// @param request 已验证请求；audio 为本地音频快照；control 为调用控制
    /// @returns 已完成的通道，失败不声称成功
    fn deliver(
        &self,
        request: &NotificationRequest,
        audio: Option<&[u8]>,
        control: &DeliveryControl,
    ) -> Result<NotificationDelivery> {
        // 1. 【通知投递】【桌面通道】按请求先完成固定桌面程序，错误直接返回
        control.check()?;
        if request.desktop {
            super::desktop::send_controlled(&request.title, &request.body, control)?;
        }
        if let Some(sound) = &request.sound {
            // 2. 【通知投递】【声音通道】只使用内置资源或已授权快照，并检查同一取消状态
            control.check()?;
            let bytes = match sound {
                NotificationSound::Builtin {
                    builtin: BuiltinSound::Alarm,
                } => include_bytes!("../assets/alarm.wav").as_slice(),
                NotificationSound::Builtin {
                    builtin: BuiltinSound::Chime,
                } => include_bytes!("../assets/reply-chime.wav").as_slice(),
                NotificationSound::File { .. } => audio
                    .ok_or_else(|| anyhow::anyhow!("notification audio snapshot is unavailable"))?,
            };
            super::playback::play(bytes, control)?;
        }
        // 3. 【通知投递】【结果确认】全部请求通道完成后，再次检查调用是否仍然有效
        control.check()?;
        Ok(NotificationDelivery {
            desktop: request.desktop,
            sound: request.sound.is_some(),
        })
    }
}
