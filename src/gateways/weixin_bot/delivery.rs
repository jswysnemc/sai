use super::client::WeixinBotClient;
use crate::gateways::channel_context::{self, ChannelContext};
use crate::paths::SaiPaths;
use anyhow::{bail, Result};
use serde_json::Value;
use std::time::Duration;

const REFRESH_WAIT: Duration = Duration::from_secs(8);

/// 【微信网关】【请求发送】参数 client/payload/paths 为客户端、单条消息和可选缓存路径，返回服务端响应。
pub(super) async fn send(
    client: &WeixinBotClient,
    payload: Value,
    paths: Option<&SaiPaths>,
) -> Result<Value> {
    send_with_wait(client, payload, paths, REFRESH_WAIT).await
}

/// 【微信网关】【刷新重试】参数含刷新等待上限 wait，返回单条消息结果；网络错误不自动重发。
async fn send_with_wait(
    client: &WeixinBotClient,
    mut payload: Value,
    paths: Option<&SaiPaths>,
    wait: Duration,
) -> Result<Value> {
    let recipient = payload["msg"]["to_user_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    if let Some(paths) = paths {
        if let Some(token) = recipient_token(paths, &recipient)? {
            payload["msg"]["context_token"] = Value::String(token);
        }
    }
    let rejected_token = payload["msg"]["context_token"].as_str().map(str::to_string);
    let response = client
        .post_json(
            "ilink/bot/sendmessage",
            payload.clone(),
            Duration::from_secs(15),
        )
        .await?;
    if response.get("ret").and_then(Value::as_i64).unwrap_or(0) == 0 {
        return Ok(response);
    }
    if !prepare_failed(&response) || paths.is_none() {
        return Err(api_error(&response));
    }
    let paths = paths.unwrap();
    // 1. 【微信网关】【上下文等待】只接受同一接收人的不同凭证，不借用其他聊天的最近凭证
    let refreshed = tokio::time::timeout(wait, async {
        loop {
            if let Some(token) = recipient_token(paths, &recipient)? {
                if Some(token.as_str()) != rejected_token.as_deref() {
                    return Ok::<_, anyhow::Error>(token);
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    let token = match refreshed {
        Ok(result) => result?,
        Err(_) => bail!("Weixin sendmessage rejected: {response}. Gateway is running but no refreshed context for this recipient arrived. Send a message to the bot in Weixin, then retry. The server error alone does not establish whether the login or context credential expired."),
    };
    // 2. 【微信网关】【单次重试】保持同一 client_id，只重发服务端明确 prepare failed 的单条请求
    payload["msg"]["context_token"] = Value::String(token);
    let response = client
        .post_json("ilink/bot/sendmessage", payload, Duration::from_secs(15))
        .await?;
    if response.get("ret").and_then(Value::as_i64).unwrap_or(0) != 0 {
        return Err(api_error(&response));
    }
    Ok(response)
}

/// 【微信网关】【明确拒绝】参数 response 为响应，返回是否为未准备成功的特定拒绝。
fn prepare_failed(response: &Value) -> bool {
    response["ret"].as_i64() == Some(-2)
        && response["errmsg"]
            .as_str()
            .is_some_and(|message| message.trim().eq_ignore_ascii_case("prepare failed"))
}

/// 【微信网关】【服务错误】参数 response 为服务端响应，返回保留真实诊断的错误。
fn api_error(response: &Value) -> anyhow::Error {
    anyhow::anyhow!("Weixin sendmessage failed: {response}")
}

/// 【微信网关】【接收人凭证】参数 paths/recipient 为缓存路径和接收人，返回匹配会话的非空凭证。
fn recipient_token(paths: &SaiPaths, recipient: &str) -> Result<Option<String>> {
    if let Some(token) = token_from(
        channel_context::load_latest_channel_context(paths, "weixin")?,
        recipient,
    ) {
        return Ok(Some(token));
    }
    let context = ChannelContext::weixin(recipient.into(), None);
    let session = crate::gateways::session::gateway_session_id(&context);
    if crate::state::locate_session_dirs(paths, &session).is_err() {
        return Ok(None);
    }
    let context = channel_context::load_session_channel_context(
        paths,
        &crate::gateways::workspace::gateway_workspace_path(paths),
        &session,
    )?;
    Ok(token_from(context, recipient))
}

/// 【微信网关】【凭证匹配】参数 context/recipient 为上下文和接收人，返回同接收人的非空凭证。
fn token_from(context: Option<ChannelContext>, recipient: &str) -> Option<String> {
    match context {
        Some(ChannelContext::Weixin {
            to_user_id,
            context_token: Some(token),
            ..
        }) if to_user_id == recipient && !token.trim().is_empty() => Some(token),
        _ => None,
    }
}

#[cfg(test)]
#[path = "delivery_tests.rs"]
mod tests;
