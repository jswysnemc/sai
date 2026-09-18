use crate::presentation::PresentationHost;
use crate::{Capabilities, EventContext, EventKind, PluginPackage, PluginRuntime};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

/// 【底栏插件】【状态快照】只提供界面已有的状态，不传入对话、密钥或宿主服务。
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TuiStatusContext {
    pub columns: usize,
    pub locale: String,
    pub mode: String,
    pub model: String,
    pub thinking: String,
    pub directory: String,
    pub context_ratio: f32,
    pub context_window_tokens: usize,
    pub cache_hit_ratio: Option<f32>,
}

/// 【底栏插件】【展示结果】左右文本由宿主着色和裁剪，插件不能写入终端转义序列。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TuiStatusLine {
    pub left: String,
    pub right: String,
}

impl TuiStatusLine {
    /// 【底栏插件】【文本校验】限制单行长度并拒绝控制字符和行分隔符。
    /// @returns 两侧均为有界单行文本时成功
    pub fn validate(&self) -> Result<()> {
        for text in [&self.left, &self.right] {
            if text.len() > 2048
                || text
                    .chars()
                    .any(|ch| ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}'))
            {
                bail!("TUI status must contain at most 2048 bytes per side without control characters");
            }
        }
        Ok(())
    }
}

/// 【底栏插件】【纯运行时】独立 VM 仅生成展示结果，不复用 Agent 状态或其他宿主授权。
pub struct TuiStatusRuntime(PluginRuntime);

impl TuiStatusRuntime {
    /// 【底栏插件】【加载】验证声明与授权，并将资源预算收窄到界面回调范围。
    /// @param package 源码快照；settings 为插件设置；granted 为用户授权
    /// @returns 有底栏监听器且没有宿主 I/O 能力的运行时
    pub fn load(
        mut package: PluginPackage,
        settings: Value,
        granted: Capabilities,
    ) -> Result<Self> {
        package.manifest.validate()?;
        granted.validate()?;
        if !package.manifest.capabilities.tui_status || !granted.tui_status {
            bail!("plugin TUI status capability is not allowed");
        }
        // 1. 【底栏插件】【预算限制】加载和每次刷新各自最多使用 100 毫秒
        let limits = &mut package.manifest.limits;
        limits.memory_bytes = limits.memory_bytes.min(4 * 1024 * 1024);
        limits.instructions = limits.instructions.min(100_000);
        limits.timeout_ms = 100;
        limits.output_bytes = limits.output_bytes.min(16 * 1024);
        limits.binary_bytes = limits.binary_bytes.min(1024 * 1024);
        limits.binary_timeout_ms = 100;
        let runtime = PluginRuntime::load(
            package,
            settings,
            Capabilities {
                tui_status: true,
                ..Default::default()
            },
            Arc::new(PresentationHost),
        )?;
        if !runtime.events().contains(&EventKind::TuiStatus) {
            bail!("TUI status plugin must register a tui_status listener");
        }
        Ok(Self(runtime))
    }

    /// 【底栏插件】【状态计算】执行有界纯回调，一个插件至多返回一组左右文本。
    /// @param context 当前底栏状态；数值必须有限，输入快照最多 16 KiB
    /// @returns 自定义底栏；返回 nil 时沿用宿主默认底栏
    pub async fn render(&self, context: &TuiStatusContext) -> Result<Option<TuiStatusLine>> {
        if !context.context_ratio.is_finite()
            || context
                .cache_hit_ratio
                .is_some_and(|ratio| !ratio.is_finite())
            || serde_json::to_vec(context)?.len() > 16 * 1024
        {
            bail!("invalid TUI status input");
        }
        let values = self
            .0
            .emit(
                EventKind::TuiStatus,
                EventContext {
                    data: serde_json::to_value(context)?,
                    ..Default::default()
                },
            )
            .await?;
        let mut output = None;
        for value in values.into_iter().filter(|value| !value.is_null()) {
            if output.is_some() {
                bail!("TUI status plugin returned multiple layouts");
            }
            let line: TuiStatusLine = serde_json::from_value(value)?;
            line.validate()?;
            output = Some(line);
        }
        Ok(output)
    }
}
