use super::channel_bridge::ChannelInboundMessage;
use super::model::{MemberRole, MemberStatus};
use super::policy::RoutingDecision;
use super::shared_service;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use anyhow::Result;
use serde_json::Value;

/// 网关入站消息经 Super-Agent 桥接层尝试处理后的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayRouteOutcome {
    /// 未启用或未命中 Super-Agent 路由，继续沿用网关原有单智能体对话路径
    PassThrough,
    /// 已由 Super-Agent 接管；`immediate_reply` 非空时由网关立即回复给来源聊天
    Handled { immediate_reply: Option<String> },
}

/// 【超级智能体】【多渠道网关入口】统一处理微信、飞书、QQ、OneBot 网关入站消息。
///
/// 当触发 `/group`、`/runners`、`/tasks` 等群组管理命令时直接返回状态文本；
/// 当启用 `super_agent.channel_leader_routing`、当前聊天已绑定群组、或消息带 `/leader` 前缀时，
/// 接入对应群组 Leader 异步编排并由 Leader 主动将成果回推至原渠道。
///
/// 参数:
/// - `paths`: Sai 运行路径
/// - `channel`: 渠道标识（weixin / feishu / qq / onebot）
/// - `external_chat_id`: 外部会话或发送人标识
/// - `actor`: 外部发送人展示名称
/// - `prompt`: 原始文本内容
/// - `reply_target`: 供 Leader 异步回推使用的结构化渠道上下文 JSON
///
/// 返回:
/// - 路由结果（接管或透传）
pub async fn try_handle_gateway_message(
    paths: &SaiPaths,
    channel: &str,
    external_chat_id: &str,
    actor: &str,
    prompt: &str,
    reply_target: Value,
) -> Result<GatewayRouteOutcome> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return Ok(GatewayRouteOutcome::PassThrough);
    }

    // 1. 优先响应渠道群组查询与绑定斜杠命令（随时可用）
    if let Some(reply) = try_handle_channel_command(paths, channel, external_chat_id, trimmed)? {
        return Ok(GatewayRouteOutcome::Handled {
            immediate_reply: Some(reply),
        });
    }

    let config = AppConfig::load_or_default(paths).unwrap_or_default();
    if !config.super_agent.enabled {
        return Ok(GatewayRouteOutcome::PassThrough);
    }

    // 2. 判断显式 `/leader` 前缀、已绑定群组或全局渠道 Leader 路由开关
    let explicit_leader = strip_leader_prefix(trimmed);
    let service = shared_service(paths)?;
    let has_binding = service
        .store()
        .resolve_channel_binding(channel, external_chat_id)?
        .is_some();
    if explicit_leader.is_none() && !has_binding && !config.super_agent.channel_leader_routing {
        return Ok(GatewayRouteOutcome::PassThrough);
    }

    let content = explicit_leader.unwrap_or(trimmed).trim();
    if content.is_empty() {
        return Ok(GatewayRouteOutcome::Handled {
            immediate_reply: Some(
                "请在 /leader 后输入要交由 Leader 编排的任务内容，或使用 /group 查看群组状态。"
                    .to_string(),
            ),
        });
    }

    // 3. 启动后台 Watchdog 守护并将渠道消息异步注入群组（全双工不阻塞网关接收循环）
    service.ensure_watchdog_started();
    let (_, _, decision) = service
        .ingest_channel_message(
            ChannelInboundMessage {
                channel: channel.to_string(),
                external_chat_id: external_chat_id.to_string(),
                actor: actor.to_string(),
                content: content.to_string(),
                group_id: None,
                reply_target,
                simulate_outbound: false,
            },
            true,
        )
        .await?;

    // 4. SlowPath 由 Leader 异步通过 deliver_to_channel 回推，避免与网关主流程重复发送
    let immediate_reply = match decision {
        RoutingDecision::FastPathWorkers { worker_ids, .. } => Some(format!(
            "已将消息直通转交给 {} 个负责该话题的 Runner，完成后由 Leader 归纳回报。",
            worker_ids.len()
        )),
        RoutingDecision::Ignore { .. } | RoutingDecision::SlowPathLeader { .. } => None,
    };
    Ok(GatewayRouteOutcome::Handled { immediate_reply })
}

/// 解析 `/leader <文本>` 或 `/群聊 <文本>` 显式路由前缀。
fn strip_leader_prefix(input: &str) -> Option<&str> {
    for prefix in ["/leader", "/群聊", "/超级代理"] {
        if let Some(rest) = input.strip_prefix(prefix) {
            if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                return Some(rest.trim_start());
            }
        }
    }
    None
}

/// 处理渠道侧的群组查询、创建、绑定与 Runner/任务状态查看命令。
fn try_handle_channel_command(
    paths: &SaiPaths,
    channel: &str,
    external_chat_id: &str,
    input: &str,
) -> Result<Option<String>> {
    let normalized = input
        .strip_prefix('/')
        .or_else(|| input.strip_prefix('／'));
    let Some(body) = normalized else {
        return Ok(None);
    };
    let mut parts = body.splitn(2, char::is_whitespace);
    let cmd = parts.next().unwrap_or_default().to_ascii_lowercase();
    let rest = parts.next().unwrap_or("").trim();

    if matches!(cmd.as_str(), "group" | "groups" | "群组") {
        let service = shared_service(paths)?;
        return Ok(Some(handle_group_subcommand(
            &service,
            channel,
            external_chat_id,
            rest,
        )?));
    }
    if matches!(cmd.as_str(), "runners" | "tasks" | "任务") {
        let service = shared_service(paths)?;
        let group = resolve_target_group(&service, channel, external_chat_id)?;
        let detail = service.group_detail(&group.id, 10)?;
        let runners: Vec<String> = detail
            .members
            .iter()
            .filter(|m| m.role == MemberRole::Worker)
            .map(|m| {
                format!(
                    "- {} [{}] {}",
                    m.name,
                    m.status.as_str(),
                    if m.last_activity.is_empty() {
                        &m.specialty
                    } else {
                        &m.last_activity
                    }
                )
            })
            .collect();
        let tasks: Vec<String> = detail
            .tasks
            .iter()
            .take(8)
            .map(|t| {
                format!(
                    "- {} [{}] ({})",
                    t.title,
                    t.status.as_str(),
                    t.assigned_member_name.as_deref().unwrap_or("unassigned")
                )
            })
            .collect();
        let mut lines = vec![format!("群组：{} ({})", group.name, group.id)];
        lines.push(format!("Runners ({}):", runners.len()));
        if runners.is_empty() {
            lines.push("- 暂无运行中的 Runner".to_string());
        } else {
            lines.extend(runners);
        }
        lines.push(format!("Tasks ({}):", detail.tasks.len()));
        if tasks.is_empty() {
            lines.push("- 暂无任务记录".to_string());
        } else {
            lines.extend(tasks);
        }
        return Ok(Some(lines.join("\n")));
    }
    Ok(None)
}

/// 分发 `/group` 子命令（list / bind / new / status）。
fn handle_group_subcommand(
    service: &super::SuperAgentService,
    channel: &str,
    external_chat_id: &str,
    rest: &str,
) -> Result<String> {
    let mut parts = rest.splitn(2, char::is_whitespace);
    let sub = parts.next().unwrap_or("").to_ascii_lowercase();
    let arg = parts.next().unwrap_or("").trim();

    match sub.as_str() {
        "" | "list" | "status" => {
            let overview = service.overview()?;
            let bound = service
                .store()
                .resolve_channel_binding(channel, external_chat_id)?;
            let mut lines = vec![format!(
                "Super-Agent 群组概览：{} 个群组，{} 个运行中 Runner，{} 份待归纳报告",
                overview.group_count, overview.running_workers, overview.pending_synthesis_reports
            )];
            if let Some(binding) = bound {
                lines.push(format!(
                    "当前聊天已绑定群组：{}（{} / {}）",
                    binding.group_id, binding.channel, binding.external_chat_id
                ));
            } else {
                lines.push(
                    "当前聊天尚未绑定群组，可发送 /group bind 绑定默认群组，或直接发送 /leader <任务>。"
                        .to_string(),
                );
            }
            for item in overview.groups.iter().take(6) {
                lines.push(format!(
                    "- {} ({}) | runners:{}/{} | open_tasks:{}",
                    item.group.name,
                    item.group.id,
                    item.running_worker_count,
                    item.worker_count,
                    item.open_task_count
                ));
            }
            Ok(lines.join("\n"))
        }
        "new" | "create" => {
            let name = if arg.is_empty() {
                format!("{channel} 协作群组")
            } else {
                arg.to_string()
            };
            let (group, _) = service.create_group(&name, "Created from channel", "", None)?;
            let _ = service.bind_channel(
                &group.id,
                channel,
                external_chat_id,
                &format!("{channel}:{external_chat_id}"),
                "auto",
                true,
            )?;
            Ok(format!(
                "已创建并绑定群组「{}」（{}）。后续消息将直接交由该群组 Leader 协调并派发 Runner 执行。",
                group.name, group.id
            ))
        }
        "bind" => {
            let group = if arg.is_empty() {
                service.ensure_default_group(None)?
            } else {
                service
                    .store()
                    .list_groups()?
                    .into_iter()
                    .find(|g| g.id == arg || g.name.eq_ignore_ascii_case(arg))
                    .ok_or_else(|| anyhow::anyhow!("未找到群组：{arg}"))?
            };
            service.bind_channel(
                &group.id,
                channel,
                external_chat_id,
                &format!("{channel}:{external_chat_id}"),
                "auto",
                true,
            )?;
            Ok(format!(
                "已将当前 {channel} 会话绑定到群组「{}」（{}）。后续消息将由 Leader 接管并主动汇报成果。",
                group.name, group.id
            ))
        }
        "unbind" => {
            if let Some(binding) = service
                .store()
                .resolve_channel_binding(channel, external_chat_id)?
            {
                service.unbind_channel(&binding.group_id, &binding.id)?;
                Ok("已解除当前聊天与 Super-Agent 群组的绑定。".to_string())
            } else {
                Ok("当前聊天未绑定任何 Super-Agent 群组。".to_string())
            }
        }
        _ => {
            let running = service
                .store()
                .list_groups()?
                .into_iter()
                .flat_map(|g| service.store().list_members(&g.id).unwrap_or_default())
                .filter(|m| m.role == MemberRole::Worker && m.status == MemberStatus::Running)
                .count();
            Ok(format!(
                "用法：\n- /group：查看群组概览（当前运行 Runner：{running}）\n- /group new <名称>：新建并绑定群组\n- /group bind [群组ID或名称]：绑定当前聊天\n- /group unbind：解除绑定\n- /runners 或 /tasks：查看当前群组 Runner 与任务\n- /leader <任务描述>：显式发给 Leader 编排"
            ))
        }
    }
}

/// 解析当前渠道聊天对应的目标群组。
fn resolve_target_group(
    service: &super::SuperAgentService,
    channel: &str,
    external_chat_id: &str,
) -> Result<super::model::Group> {
    if let Some(binding) = service
        .store()
        .resolve_channel_binding(channel, external_chat_id)?
    {
        if let Some(group) = service.store().get_group(&binding.group_id)? {
            return Ok(group);
        }
    }
    service.ensure_default_group(None)
}
