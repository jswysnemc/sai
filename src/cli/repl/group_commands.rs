use super::*;
use crate::super_agent::{shared_service, SpawnRunnerRequest};

/// 判断 `/group` 命令是否需要进入交互式备用屏工作台。
///
/// 参数:
/// - `input`: 已去除首尾空白的输入文本
///
/// 返回:
/// - 裸 `/group` 或 `/group open ...` 时返回 true
pub(super) fn opens_group_overlay(input: &str) -> bool {
    let Some(rest) = repl_command_rest(input, "/group") else {
        return false;
    };
    let trimmed = rest.trim();
    if trimmed.is_empty() {
        return true;
    }
    let sub = trimmed
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(sub.as_str(), "open" | "ui" | "tui")
}

/// 【REPL】【群组命令】处理 `/group` 及其子命令（打开群组 TUI、新建、列表、派发 Runner、发消息、查状态）。
///
/// 参数:
/// - `input`: 用户输入文本
/// - `paths`: Sai 路径集合
/// - `runtime`: 终端运行期
///
/// 返回:
/// - 命中并处理 `/group` 时返回 true
pub(super) async fn handle(
    input: &str,
    paths: &SaiPaths,
    runtime: &mut ReplRuntime,
) -> Result<bool> {
    let Some(rest) = repl_command_rest(input, "/group") else {
        return Ok(false);
    };
    let trimmed = rest.trim();
    if trimmed.is_empty() {
        open_overlay(paths, None, runtime).await?;
        return Ok(true);
    }

    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let sub = parts.next().unwrap_or("").to_ascii_lowercase();
    let arg = parts.next().unwrap_or("").trim();

    match sub.as_str() {
        "open" | "ui" | "tui" => {
            let target = (!arg.is_empty()).then_some(arg);
            open_overlay(paths, target, runtime).await?;
        }
        "list" | "ls" | "status" => {
            let service = shared_service(paths)?;
            let overview = service.overview()?;
            let mut lines = vec![format!(
                "Super-Agent 群组：{} 个 | 运行中 Runner：{}/{} | 活跃任务：{} | 待归纳报告：{}",
                overview.group_count,
                overview.running_workers,
                overview.total_workers,
                overview.active_tasks,
                overview.pending_synthesis_reports
            )];
            for item in &overview.groups {
                lines.push(format!(
                    "  - {} ({}) | leader:{} | runners:{}/{} | open_tasks:{} | channels:{}",
                    item.group.name,
                    item.group.id,
                    item.leader_status.as_str(),
                    item.running_worker_count,
                    item.worker_count,
                    item.open_task_count,
                    item.bound_channels.len()
                ));
            }
            if overview.groups.is_empty() {
                lines.push("  暂无群组；输入 /group 打开工作台或 /group new <名称> 新建。".to_string());
            }
            runtime.record_meta(lines.join("\n"))?;
        }
        "new" | "create" => {
            let name = if arg.is_empty() {
                "Super-Agent Group"
            } else {
                arg
            };
            let service = shared_service(paths)?;
            let (group, _) = service.create_group(name, "", "", None)?;
            runtime.record_meta(format!(
                "已创建群组「{}」（{}）；输入 /group open {} 进入群组对话。",
                group.name, group.id, group.id
            ))?;
        }
        "spawn" => {
            let mut spawn_parts = arg.splitn(2, char::is_whitespace);
            let name = spawn_parts.next().unwrap_or("").trim();
            let prompt = spawn_parts.next().unwrap_or("").trim();
            if name.is_empty() || prompt.is_empty() {
                runtime.record_meta("用法：/group spawn <runner名> <任务说明>".to_string())?;
                return Ok(true);
            }
            let service = shared_service(paths)?;
            service.ensure_watchdog_started();
            let group = service.ensure_default_group(None)?;
            let (member, task) = service
                .spawn_runner(SpawnRunnerRequest {
                    group_id: group.id.clone(),
                    name: name.to_string(),
                    specialty: "general".to_string(),
                    subagent_type: "general".to_string(),
                    task_title: prompt.chars().take(48).collect(),
                    prompt: prompt.to_string(),
                    thread_id: None,
                    origin_channel: Some("tui".to_string()),
                    origin_target: None,
                })
                .await?;
            runtime.record_meta(format!(
                "已在群组「{}」启动 Runner「{}」（任务 {}），输入 /group 查看实时协同。",
                group.name, member.name, task.id
            ))?;
        }
        "send" => {
            if arg.is_empty() {
                runtime.record_meta("用法：/group send <消息内容>".to_string())?;
                return Ok(true);
            }
            let service = shared_service(paths)?;
            service.ensure_watchdog_started();
            let group = service.ensure_default_group(None)?;
            let (_, decision) = service
                .post_user_message(&group.id, "You", arg, None, None, "tui", true)
                .await?;
            runtime.record_meta(format!(
                "已发送至群组「{}」（路由：{}），输入 /group 查看群内进度与回复。",
                group.name,
                decision.route_path()
            ))?;
        }
        _ => {
            runtime.record_meta(
                "用法：/group | /group list | /group new <名称> | /group open <ID|名称> | /group spawn <名> <任务> | /group send <内容>"
                    .to_string(),
            )?;
        }
    }
    Ok(true)
}

/// 【REPL】【群组备用屏】启动嵌套备用屏群组工作台并在退出后重绘 REPL。
pub(super) async fn open_overlay(
    paths: &SaiPaths,
    target_group: Option<&str>,
    runtime: &mut ReplRuntime,
) -> Result<()> {
    let result = crate::cli::repl_group::run_group_tui(paths, target_group).await;
    runtime.redraw()?;
    if let Err(err) = result {
        runtime.record_meta(format!("群组工作台错误：{err:#}"))?;
    }
    Ok(())
}
