use super::*;
use crate::plugins::commands::{run_bundled, BundledCommand};
use anyhow::Context;
use serde_json::json;

/// 【知识库命令】【兼容语法】原参数转换为插件命令，知识库业务只在 Lua 中执行
/// @param paths 应用目录；args 为旧 CLI 参数；mode 为显式权限模式
/// @returns 命令执行和标准输出结果
pub(super) async fn run_kb(paths: &SaiPaths, args: KbArgs, mode: Option<AgentMode>) -> Result<()> {
    let config = AppConfig::load(paths)?;
    let mut input = None;
    let (command, arguments) = match args.command {
        KbCommand::Add(args) => {
            let name = args
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .context("source path has no valid file or directory name")?;
            let source =
                dunce::canonicalize(&args.path).context("resolve knowledge base import source")?;
            input = Some(source.clone());
            ("add", json!({"path":source, "name":name}))
        }
        KbCommand::List => ("list", json!({})),
        KbCommand::Search(args) => (
            "search",
            json!({"query":args.query.join(" "), "limit":args.limit.map(|value|value.to_string())}),
        ),
        KbCommand::Find(args) => (
            "find",
            json!({"query":args.query.join(" "), "limit":args.limit.map(|value|value.to_string())}),
        ),
        KbCommand::Read(args) => (
            "read",
            json!({"file":args.file,"start":args.start.to_string(),"lines":args.lines.map(|value|value.to_string())}),
        ),
        KbCommand::Remove(args) => ("remove", json!({"file":args.file})),
        KbCommand::Reindex => ("reindex", json!({})),
        KbCommand::Stats => ("stats", json!({})),
        KbCommand::Embed(args) => match args.command {
            KbEmbedCommand::Reindex(args) => ("embed-reindex", json!({"quiet":args.quiet})),
        },
    };
    let result = run_bundled(
        &config,
        paths,
        BundledCommand {
            plugin: "knowledge-base",
            command,
            arguments,
            input,
            allow_writes: !matches!(mode, Some(AgentMode::Plan)),
        },
    )
    .await?;
    if !result.is_empty() {
        println!("{result}");
    }
    Ok(())
}
