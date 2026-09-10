use crate::host::{HttpRequest, HttpResponse, PluginHost};
use crate::{Capabilities, EventContext, EventKind, PluginPackage, PluginRuntime};
use anyhow::{bail, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

pub const MAX_NOTIFICATIONS: usize = 8;

/// 【插件展示】【交互面】通知仅交给正在与用户交互的终端或浏览器。
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PresentationSurface {
    Tui,
    Web,
}

/// 【插件展示】【结束状态】承载宿主已确定的结果，不允许策略改写运行状态。
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReplyStatus {
    Completed,
    Interrupted,
    Failed,
}

/// 【插件展示】【事件资料】不传入对话正文、凭据、文件路径或会话私有状态。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReplyPresentation {
    pub surface: PresentationSurface,
    pub status: ReplyStatus,
    pub locale: String,
}

impl ReplyPresentation {
    /// 【插件展示】【输入校验】限定区域代码长度及字符，不接受任意上下文载荷。
    /// @returns 区域代码合法时成功
    pub fn validate(&self) -> Result<()> {
        if self.locale.is_empty()
            || self.locale.len() > 32
            || !self
                .locale
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            bail!("notification locale must be a language code of at most 32 bytes");
        }
        Ok(())
    }
}

/// 【插件展示】【通知数据】纯回调返回展示内容，平台能力不暴露给 Lua。
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Notification {
    pub title: String,
    pub body: String,
    pub desktop: bool,
    pub sound: bool,
}

impl Notification {
    /// 【插件展示】【输出校验】限制文本大小并拒绝可执行终端控制字符。
    /// @returns 通知满足展示边界时成功；正文截断规则由业务插件决定
    pub fn validate(&self) -> Result<()> {
        if self.title.trim().is_empty() || self.title.len() > 256 || self.body.len() > 4096 {
            bail!("notification text exceeds display limits");
        }
        if self
            .title
            .chars()
            .chain(self.body.chars())
            .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
        {
            bail!("notification text contains a control character");
        }
        Ok(())
    }
}

/// 【插件展示】【纯宿主】独立展示实例不提供网络、文件、进程、模型或存储服务。
struct PresentationHost;

#[async_trait]
impl PluginHost for PresentationHost {
    /// 【插件展示】【网络隔离】拒绝展示回调访问网络，即使原清单声明了其他能力。
    /// @param request 请求；capabilities 为授权；allow_writes 为写入标志，均不用于执行
    /// @returns 固定拒绝错误
    async fn http(
        &self,
        _request: HttpRequest,
        _capabilities: Capabilities,
        _allow_writes: bool,
    ) -> Result<HttpResponse> {
        bail!("host I/O is unavailable in presentation callbacks")
    }
}

/// 【插件展示】【隔离运行】每次展示使用独立源码快照，不依赖 Agent 或连接内的 Lua 状态。
pub struct PresentationRuntime(PluginRuntime);

impl PresentationRuntime {
    /// 【插件展示】【加载】仅在通知声明与授权同时存在时构造有界纯运行时。
    /// @param package 固定源码；settings 为该插件设置；granted 为用户授权
    /// @returns 无宿主 I/O 的展示实例；加载本身也受指令与时长限制
    pub fn load(
        mut package: PluginPackage,
        settings: Value,
        granted: Capabilities,
    ) -> Result<Self> {
        package.manifest.validate()?;
        granted.validate()?;
        if !package.manifest.capabilities.notifications || !granted.notifications {
            bail!("plugin notification capability is not allowed");
        }
        // 1. 【插件展示】【资源收窄】展示不能继承调查和安装业务的长时间预算
        let limits = &mut package.manifest.limits;
        limits.memory_bytes = limits.memory_bytes.min(4 * 1024 * 1024);
        limits.instructions = limits.instructions.min(100_000);
        limits.timeout_ms = 100;
        limits.output_bytes = limits.output_bytes.min(16 * 1024);
        let capabilities = Capabilities {
            notifications: true,
            ..Default::default()
        };
        Ok(Self(PluginRuntime::load(
            package,
            settings,
            capabilities,
            Arc::new(PresentationHost),
        )?))
    }

    /// 【插件展示】【结束回调】执行只返回通知或 nil 的 reply_end 监听器。
    /// @param event 宿主传入的交互面、状态及语言
    /// @returns 已完整校验的通知列表；任一非法结果使当前插件整批失败
    pub async fn reply_end(&self, event: &ReplyPresentation) -> Result<Vec<Notification>> {
        event.validate()?;
        let results = self
            .0
            .emit(
                EventKind::ReplyEnd,
                EventContext {
                    data: serde_json::to_value(event)?,
                    ..Default::default()
                },
            )
            .await?;
        let mut notifications = Vec::new();
        for result in results.into_iter().filter(|value| !value.is_null()) {
            let notification: Notification = serde_json::from_value(result)?;
            notification.validate()?;
            if notification.desktop || notification.sound {
                notifications.push(notification);
            }
            if notifications.len() > MAX_NOTIFICATIONS {
                bail!("plugin notification count exceeds limit");
            }
        }
        Ok(notifications)
    }
}
