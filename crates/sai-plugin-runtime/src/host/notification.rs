use crate::Capabilities;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub const MAX_NOTIFICATION_AUDIO_BYTES: usize = 8 * 1024 * 1024;

/// 【插件通知】【声音资源】仅提供固定内置声音，名称不能选择任意宿主资源。
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinSound {
    Alarm,
    Chime,
}

/// 【插件通知】【声音来源】内置资源与本地路径互斥，本地文件额外要求读取授权。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum NotificationSound {
    Builtin { builtin: BuiltinSound },
    File { path: String },
}

/// 【插件通知】【投递请求】只接受有界纯文本、固定声音或已授权的本地音频。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationRequest {
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default = "desktop_default")]
    pub desktop: bool,
    #[serde(default)]
    pub sound: Option<NotificationSound>,
    #[serde(default = "timeout_default")]
    pub timeout_ms: u64,
}

/// 【插件通知】【投递结果】只有全部请求通道成功后才返回，未请求通道为 false。
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NotificationDelivery {
    pub desktop: bool,
    pub sound: bool,
}

impl NotificationRequest {
    /// 【插件通知】【边界校验】复用通知文本规则，并限制通道、音频路径和等待时长。
    /// @returns 请求可以交给宿主时成功
    pub fn validate(&self) -> Result<()> {
        if self.title.len() > 256 || self.body.len() > 4096 {
            bail!("notification text exceeds display limits");
        }
        crate::Notification {
            title: self.title.clone(),
            body: self.body.clone(),
            desktop: self.desktop,
            sound: self.sound.is_some(),
        }
        .validate()?;
        if !self.desktop && self.sound.is_none() {
            bail!("notification requires a desktop or sound channel");
        }
        if !(1..=120_000).contains(&self.timeout_ms) {
            bail!("notification timeout must be within 1-120000 milliseconds");
        }
        if let Some(NotificationSound::File { path }) = &self.sound {
            crate::capabilities::validate_read_path(path)?;
        }
        Ok(())
    }

    /// 【插件通知】【授权检查】直接投递与答复策略分别授权，音频路径继续经过读取范围校验。
    /// @param capabilities 有效授权；allow_writes 为宿主可信写入权限
    /// @returns 允许进入平台和文件检查时成功
    pub fn authorize(&self, capabilities: &Capabilities, allow_writes: bool) -> Result<()> {
        if !capabilities.system.notify {
            bail!("plugin notification delivery is not allowed");
        }
        if !allow_writes {
            bail!("read-only plugin callback cannot send notifications");
        }
        self.validate()?;
        if let Some(NotificationSound::File { path }) = &self.sound {
            capabilities.system.check_read_request(path)?;
        }
        Ok(())
    }
}

/// 【插件通知】【缺省桌面通道】未指定通道时发送桌面通知。
/// @returns true
fn desktop_default() -> bool {
    true
}

/// 【插件通知】【缺省时限】限制默认平台等待和音频播放总时长。
/// @returns 毫秒数
fn timeout_default() -> u64 {
    10_000
}
