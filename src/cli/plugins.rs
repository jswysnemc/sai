use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use crate::plugins::{self, GrantUpdate, PluginSource};
use crate::tools::ToolRegistry;
use anyhow::{bail, Result};
use clap::{Args, Subcommand};
use sai_plugin_runtime::Capabilities;
use serde_json::json;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub(crate) struct PluginsArgs {
    #[arg(long, global = true, help = "Print structured JSON")]
    pub json: bool,
    #[command(subcommand)]
    pub command: Option<PluginsCommand>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum PluginsCommand {
    /// List installed and bundled plugins
    List,
    /// Show a plugin manifest, source and effective grants
    Info { id: String },
    /// Create a Lua plugin project with a tool, command and event handler
    Init { id: String, directory: PathBuf },
    /// Validate a plugin package without installing it or granting network access
    Check { directory: PathBuf },
    /// Install a local source snapshot; new plugins remain disabled
    Install {
        directory: PathBuf,
        #[arg(long)]
        replace: bool,
    },
    /// Enable a plugin, optionally setting explicit HTTP grants
    Enable {
        id: String,
        #[arg(long, conflicts_with_all = ["allow_http", "allow_http_read_only_post", "no_http"])]
        grant_declared: bool,
        #[arg(long, value_name = "ORIGIN", conflicts_with = "no_http")]
        allow_http: Vec<String>,
        #[arg(
            long,
            value_name = "ENDPOINT",
            requires = "allow_http",
            conflicts_with = "no_http"
        )]
        allow_http_read_only_post: Vec<String>,
        #[arg(long)]
        no_http: bool,
    },
    /// Disable a plugin for subsequent loads
    Disable { id: String },
    /// Remove an installed plugin after revoking its activation and grants
    Remove { id: String },
    /// Replace a plugin's settings using a JSON file
    Configure { id: String, file: PathBuf },
    /// List commands registered by enabled plugins
    Commands,
    /// Run a plugin command through the normal permission and audit flow
    Run {
        id: String,
        command: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        arguments: Vec<String>,
    },
}

/// 【插件命令】【执行入口】管理插件无需配置模型；命令执行沿用 CLI 权限模式。
/// @param paths Sai 路径；args 为插件子命令；mode 为可选顶层权限覆盖
/// @returns 命令执行结果，失败时保留非零退出状态
pub(crate) async fn run(
    paths: &SaiPaths,
    args: PluginsArgs,
    mode: Option<crate::agent::AgentMode>,
) -> Result<()> {
    let command = args.command.unwrap_or(PluginsCommand::List);
    // 1. 【插件命令】【独立工具链】源码创建和校验不依赖模型供应商配置
    match &command {
        PluginsCommand::Init { id, directory } => {
            let path = plugins::scaffold(directory, id)?;
            return print_result(
                args.json,
                json!({"path": path}),
                &path.display().to_string(),
            );
        }
        PluginsCommand::Check { directory } => {
            let result = plugins::validate_package(directory)?;
            return print_result(
                args.json,
                serde_json::to_value(&result)?,
                &format!(
                    "{} {}: {} tools, {} commands, {} events",
                    result.manifest.id,
                    result.manifest.version,
                    result.tools.len(),
                    result.commands.len(),
                    result.events.len(),
                ),
            );
        }
        PluginsCommand::Install { directory, replace } => {
            let path = plugins::install(directory, paths, *replace)?;
            return print_result(
                args.json,
                json!({"path": path}),
                &format!("{}: {}", t("Installed", "已安装"), path.display()),
            );
        }
        _ => {}
    }
    let config = AppConfig::load_or_default(paths)?;
    match command {
        PluginsCommand::List | PluginsCommand::Info { .. } => {
            let mut found = plugins::discover(&config, paths);
            if let PluginsCommand::Info { id } = &command {
                found
                    .plugins
                    .retain(|plugin| plugin.package.manifest.id == *id);
                if found.plugins.is_empty() {
                    bail!("plugin not found: {id}");
                }
            }
            let entries = found
                .plugins
                .iter()
                .map(|plugin| {
                    json!({
                        "id": plugin.package.manifest.id,
                        "version": plugin.package.manifest.version,
                        "name": plugin.package.manifest.name,
                        "enabled": plugin.setting.enabled,
                        "source": plugin.source,
                        "manifest": plugin.runtime_manifest(),
                        "effective_grants": plugin.capabilities().intersection(&plugin.grants()),
                    })
                })
                .collect::<Vec<_>>();
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &json!({"plugins": entries, "diagnostics": found.diagnostics})
                    )?
                );
            } else {
                for plugin in &found.plugins {
                    let manifest = &plugin.package.manifest;
                    let status = if plugin.setting.enabled {
                        t("enabled", "已启用")
                    } else {
                        t("disabled", "已禁用")
                    };
                    let source = match &plugin.source {
                        PluginSource::Bundled => t("bundled", "内置").to_string(),
                        PluginSource::Installed(path) => path.display().to_string(),
                    };
                    println!("{}  {}  {status}  {source}", manifest.id, manifest.version);
                }
                if matches!(command, PluginsCommand::Info { .. }) {
                    for entry in entries {
                        println!("{}", serde_json::to_string_pretty(&entry)?);
                    }
                }
                for error in &found.diagnostics {
                    eprintln!("【插件】【发现失败】{}: {}", error.source, error.error);
                }
            }
            if !found.diagnostics.is_empty() {
                bail!(
                    "plugin discovery reported {} error(s)",
                    found.diagnostics.len()
                );
            }
            Ok(())
        }
        PluginsCommand::Enable {
            id,
            grant_declared,
            allow_http,
            allow_http_read_only_post,
            no_http,
        } => {
            let update = if grant_declared {
                GrantUpdate::Declared
            } else if no_http || !allow_http.is_empty() {
                GrantUpdate::Explicit(Capabilities {
                    http: allow_http.into_iter().collect(),
                    http_read_only_post: allow_http_read_only_post.into_iter().collect(),
                })
            } else {
                GrantUpdate::Keep
            };
            plugins::set_enabled(&config, paths, &id, true, update)?;
            updated(args.json, &id, true)
        }
        PluginsCommand::Disable { id } => {
            plugins::set_enabled(&config, paths, &id, false, GrantUpdate::Keep)?;
            updated(args.json, &id, false)
        }
        PluginsCommand::Configure { id, file } => {
            let bytes = std::fs::read(&file)?;
            if bytes.len() > 1024 * 1024 {
                bail!("plugin settings exceed 1 MiB");
            }
            plugins::configure(&config, paths, &id, serde_json::from_slice(&bytes)?)?;
            print_result(
                args.json,
                json!({"id": id, "configured": true}),
                t(
                    "Settings saved; reload the session to apply.",
                    "设置已保存，重新加载会话后生效。",
                ),
            )
        }
        PluginsCommand::Remove { id } => {
            plugins::remove(&config, paths, &id)?;
            print_result(
                args.json,
                json!({"id": id, "removed": true}),
                t("Plugin removed.", "插件已移除。"),
            )
        }
        PluginsCommand::Commands => {
            let registry = plugin_registry(&config, paths)?;
            let commands = registry
                .plugin_commands()
                .into_iter()
                .map(|(id, command)| json!({"plugin": id, "command": command}))
                .collect::<Vec<_>>();
            if args.json {
                println!("{}", serde_json::to_string_pretty(&commands)?);
            } else {
                for command in commands {
                    println!(
                        "{}/{}  {}",
                        command["plugin"].as_str().unwrap_or_default(),
                        command["command"]["name"].as_str().unwrap_or_default(),
                        command["command"]["description"]
                            .as_str()
                            .unwrap_or_default()
                    );
                }
            }
            Ok(())
        }
        PluginsCommand::Run {
            id,
            command,
            arguments,
        } => {
            let mode = mode.unwrap_or_else(|| config.permission.cli_mode().into());
            let output =
                execute_command(&config, paths, &id, &command, &arguments.join(" "), mode).await?;
            print_result(args.json, json!({"output": output}), &output)
        }
        PluginsCommand::Init { .. }
        | PluginsCommand::Check { .. }
        | PluginsCommand::Install { .. } => unreachable!(),
    }
}

/// 【插件命令】【注册检查】管理命令明确报告加载失败，避免误以为全部插件已生效。
/// @param config 主配置；paths 为 Sai 路径
/// @returns 不包含模型或 MCP 初始化的插件工具表
fn plugin_registry(config: &AppConfig, paths: &SaiPaths) -> Result<ToolRegistry> {
    let mut registry = ToolRegistry::new();
    let diagnostics = plugins::register_plugins(&mut registry, config, paths, false);
    if !diagnostics.is_empty() {
        bail!(
            "{}",
            diagnostics
                .iter()
                .map(|item| format!("{}: {}", item.source, item.error))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    Ok(registry)
}

/// 【插件命令】【权限执行】复用工具授权、审计和进度流程执行直接用户命令。
/// @param config 主配置；paths 为 Sai 路径；id、command 定位命令；arguments 为用户文本；mode 为权限模式
/// @returns 命令文本输出；用户拒绝时不执行 Lua 回调
async fn execute_command(
    config: &AppConfig,
    paths: &SaiPaths,
    id: &str,
    command: &str,
    arguments: &str,
    mode: crate::agent::AgentMode,
) -> Result<String> {
    let mut registry = plugin_registry(config, paths)?;
    let tool = registry.plugin_command(id, command)?;
    let name = tool.name.clone();
    registry.register(tool);
    registry.start_plugin_session("plugin-command")?;
    registry.set_permission_profile(crate::permission::PermissionProfile::new(
        mode.permission_profile_mode(),
        crate::runtime_cwd::current_dir()?,
        Some(crate::permission::PermissionAuditLog::new(
            paths.data_dir.join("permission-audit-cli.jsonl"),
            "plugin-command",
        )),
    ));
    let arguments = json!({"arguments": arguments}).to_string();
    if registry.requires_permission(&name, &arguments)? {
        registry.record_permission_requested(&name, &arguments)?;
        let (request, receiver) =
            crate::permission::request_permission("plugin-command", &name, &arguments);
        super::prompt_permission_request(&request)?;
        let decision = receiver.await?;
        let detail = decision.detail().map(str::to_string);
        match decision {
            crate::permission::PermissionDecision::Allow { .. } => {
                registry.record_permission_approved(&name, &arguments, detail.as_deref())?
            }
            crate::permission::PermissionDecision::Deny { reply } => {
                registry.record_permission_denied(&name, &arguments, reply.as_deref())?;
                bail!(
                    "{}",
                    reply.unwrap_or_else(|| "plugin command denied".to_string())
                );
            }
        }
    }
    registry.call(&name, &arguments).await
}

/// 【插件命令】【变更结果】说明配置已保存以及现有会话的生效方式。
/// @param structured 是否输出 JSON；id 为插件；enabled 为新状态
/// @returns 标准输出写入结果
fn updated(structured: bool, id: &str, enabled: bool) -> Result<()> {
    print_result(
        structured,
        json!({"id": id, "enabled": enabled}),
        t(
            "Saved; reload the current session or start a new one to apply.",
            "已保存；重新加载当前会话或创建新会话后生效。",
        ),
    )
}

/// 【插件命令】【结果输出】在机器可读 JSON 和简洁终端文本之间选择。
/// @param structured 是否输出 JSON；value 为结构化结果；text 为终端文本
/// @returns 输出序列化结果
fn print_result(structured: bool, value: serde_json::Value, text: &str) -> Result<()> {
    if structured {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("{text}");
    }
    Ok(())
}
