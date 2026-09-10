use super::system::paths;
use crate::notifications::delivery::{
    DeliveryControl, NotificationBackend, SystemNotificationBackend,
};
use anyhow::{bail, Context, Result};
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::OpenOptions;
use sai_plugin_runtime::host::{
    NotificationDelivery, NotificationRequest, NotificationSound, SystemContext,
    MAX_NOTIFICATION_AUDIO_BYTES,
};
use sai_plugin_runtime::Capabilities;
use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

struct CancelDelivery(DeliveryControl);

impl Drop for CancelDelivery {
    /// 【插件通知】【取消传递】宿主 Future 结束或取消时通知工作线程停止。
    /// @returns 无
    fn drop(&mut self) {
        self.0.cancel();
    }
}

/// 【插件通知】【宿主投递】绑定正式平台后端，读取与投递使用同一时限和取消状态。
/// @param request 请求；context 为可信目录与权限；capabilities 为有效授权
/// @returns 平台投递结果
pub(super) async fn send(
    request: NotificationRequest,
    context: SystemContext,
    capabilities: Capabilities,
) -> Result<NotificationDelivery> {
    send_with_backend(
        request,
        context,
        capabilities,
        Arc::new(SystemNotificationBackend),
    )
    .await
}

/// 【插件通知】【受限执行】先复核权限并读取音频快照，再进入可以替换的平台边界。
/// @param request 请求；context 为可信目录与权限；capabilities 为授权；backend 为平台实现
/// @returns 投递结果，取消与超时会通知阻塞工作线程
pub(super) async fn send_with_backend(
    request: NotificationRequest,
    context: SystemContext,
    capabilities: Capabilities,
    backend: Arc<dyn NotificationBackend>,
) -> Result<NotificationDelivery> {
    // 1. 【插件通知】【调用控制】宿主复核授权，将同一取消标志交给阻塞工作线程
    request.authorize(&capabilities, context.allow_writes)?;
    let timeout = Duration::from_millis(request.timeout_ms);
    let control = DeliveryControl::new(timeout);
    let _cancel = CancelDelivery(control.clone());
    // 2. 【插件通知】【投递执行】先读取受限快照，再进入平台后端，阶段之间检查取消
    let task = tokio::task::spawn_blocking(move || {
        control.check()?;
        let audio = read_audio(&request, &context, &capabilities, &control)?;
        control.check()?;
        let result = backend.deliver(&request, audio.as_deref(), &control)?;
        control.check()?;
        Ok::<_, anyhow::Error>(result)
    });
    // 3. 【插件通知】【等待结果】时限到达或等待被取消时，通过守卫停止后续投递
    tokio::time::timeout(timeout, task)
        .await
        .context("plugin notification delivery timed out")?
        .context("notification delivery worker stopped")?
}

/// 【插件通知】【音频快照】通过授权目录句柄读取普通文件，禁止设备、管道和越界链接。
/// @param request 请求；context 为可信目录；capabilities 为读取授权；control 为调用控制
/// @returns 本地音频快照，内置声音和无声音请求返回 None
fn read_audio(
    request: &NotificationRequest,
    context: &SystemContext,
    capabilities: &Capabilities,
    control: &DeliveryControl,
) -> Result<Option<Vec<u8>>> {
    let Some(NotificationSound::File { path }) = &request.sound else {
        return Ok(None);
    };
    // 1. 【插件通知】【文件授权】固定真实路径和目录句柄，末级打开不跟随替换链接
    let path = paths::authorize(path, context, capabilities)?;
    control.check()?;
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = path
        .directory
        .open_with(&path.relative, &options)
        .context("open notification audio")?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        bail!("notification audio requires a regular file");
    }
    if metadata.len() > MAX_NOTIFICATION_AUDIO_BYTES as u64 {
        bail!("notification audio exceeds 8 MiB");
    }
    // 2. 【插件通知】【分块快照】每块检查取消和实际字节数，防止读取期间文件增长
    let mut reader = file.take(MAX_NOTIFICATION_AUDIO_BYTES as u64 + 1);
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        control.check()?;
        let count = reader.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.len() > MAX_NOTIFICATION_AUDIO_BYTES {
            bail!("notification audio exceeds 8 MiB");
        }
    }
    if bytes.is_empty() {
        bail!("notification audio file is empty");
    }
    Ok(Some(bytes))
}
