use crate::paths::SaiPaths;
use crate::super_agent::shared_service;
use anyhow::Result;
use clap::{Args, Subcommand};

/// `sai group` 子命令参数：默认进入交互式群组 TUI，也支持列出、新建、发消息与查状态。
#[derive(Debug, Args)]
pub struct GroupArgs {
    /// 可选直接打开的目标群组 ID 或名称
    #[arg(long)]
    pub group: Option<String>,

    #[command(subcommand)]
    pub command: Option<GroupSubcommand>,
}

#[derive(Debug, Subcommand)]
pub enum GroupSubcommand {
    /// 列出全部 Super-Agent 群组及运行概览
    List,
    /// 创建新协作群组
    New {
        /// 群组名称
        name: String,
        /// 群组说明
        #[arg(long, default_value = "")]
        description: String,
    },
    /// 向指定群组发送一条消息并交由 Leader/Runner 处理
    Send {
        /// 目标群组 ID 或名称
        group: String,
        /// 消息正文
        #[arg(trailing_var_arg = true, required = true)]
        message: Vec<String>,
    },
    /// 查看指定群组（或默认群组）详情状态
    Status {
        /// 可选群组 ID 或名称
        group: Option<String>,
    },
}

/// 【CLI】【群组命令】执行 `sai group` 子命令入口。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `args`: 命令行参数
///
/// 返回:
/// - 执行结果
pub(super) async fn run(paths: &SaiPaths, args: GroupArgs) -> Result<()> {
    match args.command {
        None => crate::cli::repl_group::run_group_tui(paths, args.group.as_deref()).await,
        Some(GroupSubcommand::List) => {
            let service = shared_service(paths)?;
            let overview = service.overview()?;
            println!(
                "Groups: {} | Running Runners: {}/{} | Active Tasks: {} | Pending Reports: {}",
                overview.group_count,
                overview.running_workers,
                overview.total_workers,
                overview.active_tasks,
                overview.pending_synthesis_reports
            );
            for item in overview.groups {
                println!(
                    "- {} ({}) | leader:{} | runners:{}/{} | open_tasks:{} | channels:{}",
                    item.group.name,
                    item.group.id,
                    item.leader_status.as_str(),
                    item.running_worker_count,
                    item.worker_count,
                    item.open_task_count,
                    item.bound_channels.len()
                );
            }
            Ok(())
        }
        Some(GroupSubcommand::New { name, description }) => {
            let service = shared_service(paths)?;
            let (group, leader) = service.create_group(&name, &description, "", None)?;
            println!(
                "Created group '{}' ({}) with leader '{}' ({})",
                group.name, group.id, leader.name, leader.id
            );
            Ok(())
        }
        Some(GroupSubcommand::Send { group, message }) => {
            let service = shared_service(paths)?;
            service.ensure_watchdog_started();
            let target = resolve_group(&service, Some(&group))?;
            let text = message.join(" ");
            let (msg, decision) = service
                .post_user_message(&target.id, "You", &text, None, None, "cli", true)
                .await?;
            println!(
                "Sent message {} to '{}' (route: {})",
                msg.id,
                target.name,
                decision.route_path()
            );
            Ok(())
        }
        Some(GroupSubcommand::Status { group }) => {
            let service = shared_service(paths)?;
            let target = resolve_group(&service, group.as_deref())?;
            let detail = service.group_detail(&target.id, 15)?;
            println!("Group: {} ({})", detail.group.name, detail.group.id);
            println!("Workspace: {}", detail.group.workspace_path);
            println!("Members ({}):", detail.members.len());
            for member in &detail.members {
                println!(
                    "  - [{}] {} ({}) {}",
                    member.role.as_str(),
                    member.name,
                    member.status.as_str(),
                    member.last_activity
                );
            }
            println!("Tasks ({}):", detail.tasks.len());
            for task in detail.tasks.iter().take(10) {
                println!(
                    "  - {} [{}] ({})",
                    task.title,
                    task.status.as_str(),
                    task.assigned_member_name.as_deref().unwrap_or("-")
                );
            }
            println!("Channels ({}):", detail.channels.len());
            for ch in &detail.channels {
                println!(
                    "  - {}:{} ({})",
                    ch.channel, ch.external_chat_id, ch.admission_mode
                );
            }
            Ok(())
        }
    }
}

/// 按 ID 或名称查找群组，缺省时使用默认群组。
fn resolve_group(
    service: &crate::super_agent::SuperAgentService,
    target: Option<&str>,
) -> Result<crate::super_agent::model::Group> {
    if let Some(key) = target {
        if let Some(found) = service
            .store()
            .list_groups()?
            .into_iter()
            .find(|g| g.id == key || g.name.eq_ignore_ascii_case(key))
        {
            return Ok(found);
        }
        anyhow::bail!("group not found: {key}");
    }
    service.ensure_default_group(None)
}
