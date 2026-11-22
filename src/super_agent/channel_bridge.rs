use super::model::ChannelDeliveryReceipt;
use super::store::SuperAgentStore;
use crate::config::AppConfig;
use crate::gateways::channel_context::{load_latest_channel_context, ChannelContext};
use crate::gateways::channel_target::{resolve_channel_target, ActiveChannelTarget};
use crate::gateways::feishu::FeishuClient;
use crate::gateways::message::OutboundMessage;
use crate::gateways::onebot::{OneBotClient, OneBotTargetKind};
use crate::paths::SaiPaths;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

/// 渠道入站消息统一描述符（微信 / 飞书 / QQ / OneBot / 模拟渠道共用）。
#[derive(Debug, Clone)]
pub struct ChannelInboundMessage {
    /// 渠道名：weixin / feishu / qq / onebot / web / tui
    pub channel: String,
    /// 外部聊天/用户/群标识
    pub external_chat_id: String,
    /// 发送人名称或 ID
    pub actor: String,
    /// 文本正文
    pub content: String,
    /// 可选显式指定的目标群组 ID
    pub group_id: Option<String>,
    /// 附带给回推路由使用的结构化目标上下文
    pub reply_target: Value,
    /// 为 true 时仅模拟渠道回推而不调用真实外部平台接口
    pub simulate_outbound: bool,
}

/// 【超级智能体】【渠道格式】按渠道信息密度约束裁剪 IM 输出（对应论文 4.2 节）。
///
/// 参数:
/// - `channel`: 渠道名称
/// - `content`: Leader 归纳后的原始成果文本
/// - `max_words`: IM 渠道建议字数上限
///
/// 返回:
/// - 适配该渠道展示习惯的文本
pub fn format_for_channel(channel: &str, content: &str, max_words: usize) -> String {
    let trimmed = content.trim();
    let lower = channel.trim().to_ascii_lowercase();
    if !matches!(lower.as_str(), "weixin" | "wechat" | "feishu" | "qq" | "onebot") {
        return trimmed.to_string();
    }
    // 1. 飞书与微信/QQ 文本通道去除多余围栏标记，保持干净易读
    let plain = if lower == "feishu" {
        trimmed.replace("```", "")
    } else {
        trimmed.to_string()
    };
    let char_budget = max_words.saturating_mul(3).max(160);
    let mut chars = plain.chars();
    let head: String = chars.by_ref().take(char_budget).collect();
    if chars.next().is_some() {
        format!("{head}...\n（详细成果已同步至 Sai 群组面板）")
    } else {
        head
    }
}

/// 【超级智能体】【渠道递交】将 Leader 归纳后的成果主动推送回来源渠道，并记录递交回执。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `store`: Super-Agent 存储
/// - `group_id`: 所属群组标识
/// - `task_id`: 关联任务标识
/// - `channel`: 来源渠道名称
/// - `target`: 任务保存的渠道回复目标 JSON
/// - `content`: Leader 归纳后的成果正文
///
/// 返回:
/// - 渠道递交回执记录
pub async fn deliver_to_channel(
    paths: &SaiPaths,
    store: &SuperAgentStore,
    group_id: &str,
    task_id: Option<&str>,
    channel: &str,
    target: &Value,
    content: &str,
) -> Result<ChannelDeliveryReceipt> {
    let normalized = channel.trim().to_ascii_lowercase();
    let config = AppConfig::load_or_default(paths).unwrap_or_default();
    let formatted = format_for_channel(
        &normalized,
        content,
        config.super_agent.im_reply_max_words,
    );
    let preview = preview_text(&formatted, 140);
    let target_label = describe_target(&normalized, target);

    // 1. 本地 TUI/Web/Group 渠道仅需群组内展示，直接登记已送达回执
    if matches!(normalized.as_str(), "" | "group" | "tui" | "web") {
        return store.record_delivery_receipt(
            group_id,
            task_id,
            if normalized.is_empty() {
                "group"
            } else {
                &normalized
            },
            &target_label,
            "delivered_in_group",
            &preview,
            None,
        );
    }

    // 2. 模拟渠道模式不外发真实网络请求，便于本地端到端演练与验收
    if target
        .get("simulate")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        touch_matching_binding(store, &normalized, target);
        return store.record_delivery_receipt(
            group_id,
            task_id,
            &normalized,
            &target_label,
            "simulated",
            &preview,
            None,
        );
    }

    // 3. 真实外部渠道推送（Weixin / Feishu / QQ / OneBot）
    let send_result = match normalized.as_str() {
        "weixin" | "wechat" => send_weixin(paths, &config, target, &formatted).await,
        "feishu" => send_feishu(paths, &config, target, &formatted).await,
        "qq" => send_qq(paths, &config, target, &formatted).await,
        "onebot" => send_onebot(target, &formatted).await,
        other => Err(anyhow!("unsupported outbound channel: {other}")),
    };
    touch_matching_binding(store, &normalized, target);
    match send_result {
        Ok(()) => store.record_delivery_receipt(
            group_id,
            task_id,
            &normalized,
            &target_label,
            "sent",
            &preview,
            None,
        ),
        Err(error) => {
            let reason = format!("{error:#}");
            store.record_delivery_receipt(
                group_id,
                task_id,
                &normalized,
                &target_label,
                "failed",
                &preview,
                Some(&reason),
            )
        }
    }
}

/// 通过微信 iLink 主动推送成果。
async fn send_weixin(
    paths: &SaiPaths,
    config: &AppConfig,
    target: &Value,
    text: &str,
) -> Result<()> {
    let context = if let Some(user_id) = target
        .get("to_user_id")
        .or_else(|| target.get("external_chat_id"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        let token = target
            .get("context_token")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        ChannelContext::weixin(user_id.to_string(), token)
    } else {
        load_latest_channel_context(paths, "weixin")?
            .ok_or_else(|| anyhow!("no recent Weixin recipient context available"))?
    };
    match resolve_channel_target(paths, config, &context).await? {
        ActiveChannelTarget::Weixin {
            client,
            to_user_id,
            context_token,
        } => {
            client
                .with_delivery_paths(paths.clone())
                .send_text(&to_user_id, text, context_token.as_deref())
                .await
        }
        _ => Err(anyhow!("resolved target is not Weixin")),
    }
}

/// 通过飞书开放平台主动推送成果。
async fn send_feishu(
    paths: &SaiPaths,
    config: &AppConfig,
    target: &Value,
    text: &str,
) -> Result<()> {
    let chat_id = target
        .get("chat_id")
        .or_else(|| target.get("external_chat_id"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            load_latest_channel_context(paths, "feishu")
                .ok()
                .flatten()
                .and_then(|ctx| match ctx {
                    ChannelContext::Feishu { chat_id, .. } => Some(chat_id),
                    _ => None,
                })
        })
        .ok_or_else(|| anyhow!("no Feishu chat_id available for outbound delivery"))?;
    let feishu = &config.gateways.feishu;
    if feishu.app_id.trim().is_empty() || feishu.app_secret.trim().is_empty() {
        return Err(anyhow!("Feishu gateway app_id/app_secret is not configured"));
    }
    let client = FeishuClient::new(&feishu.base_url, &feishu.app_id, &feishu.app_secret);
    client.send_text(&chat_id, text).await
}

/// 通过 QQ 官方机器人主动推送成果。
async fn send_qq(paths: &SaiPaths, config: &AppConfig, target: &Value, text: &str) -> Result<()> {
    let context = if let Some(target_id) = target
        .get("target_id")
        .or_else(|| target.get("external_chat_id"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        let kind_name = target
            .get("target_kind")
            .and_then(Value::as_str)
            .unwrap_or("user");
        let kind = crate::gateways::qq_official::QqTargetKind::parse(kind_name)?;
        let msg_id = target
            .get("msg_id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        ChannelContext::qq(kind, target_id.to_string(), msg_id)
    } else {
        load_latest_channel_context(paths, "qq")?
            .ok_or_else(|| anyhow!("no recent QQ recipient context available"))?
    };
    match resolve_channel_target(paths, config, &context).await? {
        ActiveChannelTarget::Qq { client, msg_id } => {
            let outbound = OutboundMessage {
                text: Some(text.to_string()),
                media: Vec::new(),
            };
            let _ = client.send(&outbound, msg_id.as_deref()).await?;
            Ok(())
        }
        _ => Err(anyhow!("resolved target is not QQ")),
    }
}

/// 通过 OneBot HTTP 接口主动推送成果。
async fn send_onebot(target: &Value, text: &str) -> Result<()> {
    let base_url = target
        .get("onebot_base_url")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("missing onebot_base_url in channel target"))?;
    let target_kind = target
        .get("target_kind")
        .and_then(Value::as_str)
        .unwrap_or("private");
    let target_id = target
        .get("target_id")
        .and_then(Value::as_i64)
        .or_else(|| {
            target
                .get("external_chat_id")
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<i64>().ok())
        })
        .ok_or_else(|| anyhow!("missing numeric OneBot target_id"))?;
    let access_token = target
        .get("access_token")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let client = OneBotClient::new(
        base_url.to_string(),
        access_token,
        OneBotTargetKind::parse(target_kind)?,
        target_id,
    );
    let outbound = OutboundMessage {
        text: Some(text.to_string()),
        media: Vec::new(),
    };
    let _ = client.send(&outbound).await?;
    Ok(())
}

/// 刷新匹配渠道绑定的最近出站时间。
fn touch_matching_binding(store: &SuperAgentStore, channel: &str, target: &Value) {
    let chat_id = target
        .get("external_chat_id")
        .or_else(|| target.get("chat_id"))
        .or_else(|| target.get("to_user_id"))
        .or_else(|| target.get("target_id"))
        .and_then(Value::as_str)
        .unwrap_or("*");
    if let Ok(Some(binding)) = store.resolve_channel_binding(channel, chat_id) {
        let _ = store.touch_channel_binding(&binding.id, false);
    }
}

/// 构造可读的目标标签。
fn describe_target(channel: &str, target: &Value) -> String {
    let chat_id = target
        .get("external_chat_id")
        .or_else(|| target.get("chat_id"))
        .or_else(|| target.get("to_user_id"))
        .or_else(|| target.get("target_id"))
        .and_then(Value::as_str)
        .unwrap_or("default");
    if channel.is_empty() {
        "group".to_string()
    } else {
        format!("{channel}:{chat_id}")
    }
}

/// 截取回执预览文本。
fn preview_text(text: &str, limit: usize) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = compact.chars();
    let head: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        format!("{head}...")
    } else {
        head
    }
}

/// 构造标准渠道回复目标 JSON。
///
/// 参数:
/// - `external_chat_id`: 外部会话标识
/// - `simulate`: 是否模拟出站
///
/// 返回:
/// - 结构化 JSON 目标
pub fn default_channel_target(external_chat_id: &str, simulate: bool) -> Value {
    json!({
        "external_chat_id": external_chat_id,
        "simulate": simulate,
    })
}
