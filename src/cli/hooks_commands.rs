//! Hooks 来源审查和执行记录查询，授权必须显式确认内容摘要。
use crate::{config::AppConfig, paths::SaiPaths};
use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub(crate) struct HooksArgs {
    #[command(subcommand)]
    command: HooksCommand,
}
#[derive(Debug, Subcommand)]
enum HooksCommand {
    /// 显示发现的定义、来源与待授权内容摘要
    List,
    /// 授权已经审查的来源；必须提供 list 中显示的摘要
    Trust {
        source: String,
        #[arg(long)]
        digest: String,
    },
    /// 查询最近执行记录
    History {
        #[arg(long)]
        session: Option<String>,
    },
}

/// 【Hooks】【命令入口】参数为路径和命令，返回预览、授权或记录查询结果
pub(crate) fn run(paths: &SaiPaths, args: HooksArgs) -> Result<()> {
    let config = AppConfig::load_or_default(paths)?;
    match args.command {
        HooksCommand::List => println!(
            "{}",
            serde_json::to_string_pretty(&crate::hooks::sources::preview(
                &config,
                paths,
                &crate::runtime_cwd::current_dir()?
            )?)?
        ),
        HooksCommand::Trust { source, digest } => {
            let file = std::path::PathBuf::from(source)
                .canonicalize()?
                .display()
                .to_string();
            let found = crate::hooks::sources::preview(
                &config,
                paths,
                &crate::runtime_cwd::current_dir()?,
            )?
            .into_iter()
            .find(|source| source.path == file)
            .ok_or_else(|| anyhow::anyhow!("source is not an active hook configuration"))?;
            crate::hooks::trust::approve(paths, &found, &digest)?;
            println!("Hook source trusted: {}", found.path);
        }
        HooksCommand::History { session } => println!(
            "{}",
            serde_json::to_string_pretty(&crate::hooks::history::recent(
                paths,
                session.as_deref()
            ))?
        ),
    }
    Ok(())
}
