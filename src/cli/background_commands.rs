use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::tools::command::{
    background_timeout::timeout_seconds_from_cli, cleanup_background_tasks_for_user,
    list_background_tasks_for_user, read_background_task_output_for_user,
    start_background_task_for_user, stop_background_task_for_user,
};
use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct BackgroundCommandsArgs {
    #[command(subcommand)]
    pub command: BackgroundCommand,
}

#[derive(Debug, Subcommand)]
pub enum BackgroundCommand {
    Start(BackgroundCommandStartArgs),
    List,
    Output(BackgroundCommandOutputArgs),
    Stop(BackgroundCommandStopArgs),
    Cleanup(BackgroundCommandCleanupArgs),
}

#[derive(Debug, Args)]
pub struct BackgroundCommandStartArgs {
    #[arg(long)]
    pub cwd: Option<String>,
    #[arg(long)]
    pub label: Option<String>,
    #[arg(long)]
    pub timeout_seconds: Option<u64>,
    #[arg(long)]
    pub no_timeout: bool,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
    pub command: Vec<String>,
}

#[derive(Debug, Args)]
pub struct BackgroundCommandOutputArgs {
    pub task_id: String,
    #[arg(long, default_value = "all")]
    pub stream: String,
    #[arg(long, default_value_t = 200)]
    pub tail_lines: usize,
}

#[derive(Debug, Args)]
pub struct BackgroundCommandStopArgs {
    pub task_id: String,
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct BackgroundCommandCleanupArgs {
    #[arg(long)]
    pub remove_logs: bool,
}

/// 执行后台命令管理 CLI。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `args`: CLI 参数
///
/// 返回:
/// - 执行是否成功
pub async fn run_background_commands(paths: &SaiPaths, args: BackgroundCommandsArgs) -> Result<()> {
    match args.command {
        BackgroundCommand::Start(args) => {
            AppConfig::init_files(paths)?;
            let config = AppConfig::load_or_default(paths)?;
            let timeout_seconds = timeout_seconds_from_cli(args.timeout_seconds, args.no_timeout)?;
            println!(
                "{}",
                start_background_task_for_user(
                    paths,
                    &config,
                    &args.command.join(" "),
                    args.cwd.as_deref(),
                    args.label.as_deref(),
                    timeout_seconds,
                )?
            );
        }
        BackgroundCommand::List => {
            AppConfig::init_files(paths)?;
            let config = AppConfig::load_or_default(paths)?;
            println!("{}", list_background_tasks_for_user(paths, &config).await?);
        }
        BackgroundCommand::Output(args) => {
            AppConfig::init_files(paths)?;
            let config = AppConfig::load_or_default(paths)?;
            println!(
                "{}",
                read_background_task_output_for_user(
                    paths,
                    &config,
                    &args.task_id,
                    &args.stream,
                    args.tail_lines,
                )
                .await?
            );
        }
        BackgroundCommand::Stop(args) => {
            AppConfig::init_files(paths)?;
            let config = AppConfig::load_or_default(paths)?;
            println!(
                "{}",
                stop_background_task_for_user(paths, &config, &args.task_id, args.force).await?
            );
        }
        BackgroundCommand::Cleanup(args) => {
            AppConfig::init_files(paths)?;
            let config = AppConfig::load_or_default(paths)?;
            println!(
                "{}",
                cleanup_background_tasks_for_user(paths, &config, args.remove_logs).await?
            );
        }
    }
    Ok(())
}
