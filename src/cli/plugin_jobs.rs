use crate::{
    paths::SaiPaths,
    plugins::{legacy_alarm_jobs, scheduler},
};
use anyhow::{bail, Result};
use clap::{Args, Subcommand};
use serde_json::json;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub(crate) struct JobsArgs {
    pub plugin: String,
    #[command(subcommand)]
    pub command: JobCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum JobCommand {
    /// List this plugin's scheduled task records
    List,
    /// Cancel a pending or executing task without loading plugin code
    Cancel { id: String },
    /// Resume an unstarted task whose worker stopped; never replay started work
    Resume { id: String },
}

#[derive(Debug, Args)]
pub struct WorkerArgs {
    #[arg(long)]
    pub state_dir: PathBuf,
    #[arg(long)]
    pub plugin: String,
    #[arg(long)]
    pub id: String,
    #[arg(long)]
    pub launch: String,
}

/// 【插件调度】【管理命令】任务读取和取消不依赖插件仍然启用，恢复重新检查授权。
/// @param paths 应用路径；args 为管理操作
/// @returns JSON 输出结果
pub(crate) fn run(paths: &SaiPaths, args: &JobsArgs) -> Result<()> {
    let value = match &args.command {
        JobCommand::List => {
            let mut tasks = scheduler::list(paths, &args.plugin)?;
            if args.plugin == "alarm" {
                tasks.extend(legacy_alarm_jobs::list(paths)?);
                tasks.sort_by(|left, right| {
                    (left.created_at, &left.id).cmp(&(right.created_at, &right.id))
                });
            }
            json!({"jobs":tasks})
        }
        JobCommand::Cancel { id } => {
            let cancelled =
                if args.plugin == "alarm" && scheduler::get(paths, &args.plugin, id)?.is_none() {
                    legacy_alarm_jobs::cancel(paths, id)?
                } else {
                    scheduler::cancel(paths, &args.plugin, id)?
                };
            json!({"id":id,"cancelled":cancelled})
        }
        JobCommand::Resume { id } => {
            if args.plugin == "alarm"
                && scheduler::get(paths, &args.plugin, id)?.is_none()
                && legacy_alarm_jobs::get(paths, id)?.is_some()
            {
                bail!("legacy alarms cannot be resumed or replayed");
            }
            json!({"task":scheduler::resume_task(paths, &args.plugin, id)?})
        }
    };
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
