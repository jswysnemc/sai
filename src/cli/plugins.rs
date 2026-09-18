use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use crate::plugins::{self, GrantChanges, GrantUpdate, PluginSource};
use crate::tools::ToolRegistry;
use anyhow::{bail, Result};
use clap::{Args, Subcommand};
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
    /// List explicitly installed plugins
    List,
    /// Show a plugin manifest, source and effective grants
    Info { id: String },
    /// Create a Lua plugin project with a tool, command and event handler
    Init { id: String, directory: PathBuf },
    /// Validate a plugin package without installing it or granting network access
    Check { directory: PathBuf },
    /// 【插件命令】【源码分发】校验并打包清单和 Lua，不包含用户配置或授权
    #[command(about = "Pack a validated Lua source snapshot into a tar.gz archive")]
    Pack {
        directory: PathBuf,
        #[arg(
            short,
            long,
            value_name = "ARCHIVE",
            help = "Output .tar.gz or .tgz path; defaults to ID-VERSION.tar.gz"
        )]
        output: Option<PathBuf>,
    },
    /// Install a local source snapshot; new plugins remain disabled
    Install {
        directory: PathBuf,
        #[arg(long)]
        replace: bool,
    },
    /// 【插件命令】【启用授权】启用插件，并按声明分别调整各项宿主能力授权
    Enable {
        id: String,
        #[arg(long, conflicts_with_all = ["allow_http", "allow_http_read_any", "no_http_read_any", "allow_http_read_only_post", "no_http", "allow_model", "no_model", "allow_vision", "no_vision", "allow_tool", "no_tools", "allow_read_path", "no_file_read", "allow_remove_path", "no_file_remove", "allow_trash_path", "no_file_trash", "allow_env", "no_env", "allow_process", "no_processes", "allow_session_storage", "no_session_storage", "allow_plugin_storage", "no_plugin_storage", "allow_workspace", "no_workspace", "allow_notifications", "no_notifications", "allow_tui_status", "no_tui_status", "allow_reply_policy", "no_reply_policy", "allow_notify", "no_notify", "allow_schedule", "no_schedule", "allow_public_downloads", "no_public_downloads", "allow_write_path", "no_file_write", "allow_image_display", "no_image_display"])]
        grant_declared: bool,
        #[arg(long, value_name = "ORIGIN", conflicts_with = "no_http")]
        allow_http: Vec<String>,
        #[arg(long, conflicts_with_all = ["no_http", "no_http_read_any"], help = "Allow GET and HEAD to any HTTP(S) origin, including local services")]
        allow_http_read_any: bool,
        #[arg(
            long,
            help = "Revoke arbitrary-origin GET and HEAD without changing exact origins"
        )]
        no_http_read_any: bool,
        #[arg(
            long,
            value_name = "ENDPOINT",
            requires = "allow_http",
            conflicts_with = "no_http"
        )]
        allow_http_read_only_post: Vec<String>,
        #[arg(long)]
        no_http: bool,
        #[arg(long, conflicts_with = "no_model")]
        allow_model: bool,
        #[arg(long)]
        no_model: bool,
        #[arg(long, conflicts_with = "no_vision")]
        allow_vision: bool,
        #[arg(long)]
        no_vision: bool,
        #[arg(long, conflicts_with = "no_notifications")]
        allow_notifications: bool,
        #[arg(long)]
        no_notifications: bool,
        #[arg(
            long,
            conflicts_with = "no_tui_status",
            help = "Allow a pure TUI status line renderer"
        )]
        allow_tui_status: bool,
        #[arg(long, help = "Revoke TUI status line customization")]
        no_tui_status: bool,
        #[arg(long, conflicts_with = "no_reply_policy")]
        allow_reply_policy: bool,
        #[arg(long)]
        no_reply_policy: bool,
        #[arg(
            long,
            conflicts_with = "no_notify",
            help = "Allow direct desktop and audio notifications"
        )]
        allow_notify: bool,
        #[arg(long, help = "Revoke direct notification delivery")]
        no_notify: bool,
        #[arg(
            long,
            conflicts_with = "no_schedule",
            help = "Allow persistent background plugin commands"
        )]
        allow_schedule: bool,
        #[arg(long, help = "Revoke persistent plugin scheduling")]
        no_schedule: bool,
        #[arg(long, value_name = "NAME", conflicts_with = "no_tools")]
        allow_tool: Vec<String>,
        #[arg(long)]
        no_tools: bool,
        #[arg(long, value_name = "PATH", conflicts_with = "no_file_read")]
        allow_read_path: Vec<String>,
        #[arg(long)]
        no_file_read: bool,
        #[arg(
            long,
            value_name = "PATH",
            conflicts_with = "no_file_remove",
            help = "Allow permanent file removal inside a directory"
        )]
        allow_remove_path: Vec<String>,
        #[arg(long, help = "Revoke permanent file removal")]
        no_file_remove: bool,
        #[arg(
            long,
            value_name = "PATH",
            conflicts_with = "no_file_trash",
            help = "Allow moving files from a directory into the system trash"
        )]
        allow_trash_path: Vec<String>,
        #[arg(long, help = "Revoke moving files into the system trash")]
        no_file_trash: bool,
        #[arg(long, value_name = "NAME", conflicts_with = "no_env")]
        allow_env: Vec<String>,
        #[arg(long)]
        no_env: bool,
        #[arg(long, value_name = "TEMPLATE", conflicts_with = "no_processes")]
        allow_process: Vec<String>,
        #[arg(long)]
        no_processes: bool,
        #[arg(long, conflicts_with = "no_session_storage")]
        allow_session_storage: bool,
        #[arg(long)]
        no_session_storage: bool,
        #[arg(long, conflicts_with = "no_plugin_storage")]
        allow_plugin_storage: bool,
        #[arg(long)]
        no_plugin_storage: bool,
        #[arg(long, conflicts_with = "no_workspace")]
        allow_workspace: bool,
        #[arg(long)]
        no_workspace: bool,
        #[arg(long, conflicts_with = "no_public_downloads")]
        allow_public_downloads: bool,
        #[arg(long)]
        no_public_downloads: bool,
        #[arg(long, value_name = "PATH", conflicts_with = "no_file_write")]
        allow_write_path: Vec<String>,
        #[arg(long)]
        no_file_write: bool,
        #[arg(long, conflicts_with = "no_image_display")]
        allow_image_display: bool,
        #[arg(long)]
        no_image_display: bool,
    },
    /// Disable a plugin for subsequent loads
    Disable { id: String },
    /// Remove an installed plugin after revoking its activation and grants
    Remove { id: String },
    /// Replace a plugin's settings using a JSON file
    Configure { id: String, file: PathBuf },
    /// List commands registered by enabled plugins
    Commands,
    /// Manage persistent scheduled plugin tasks
    Jobs(super::plugin_jobs::JobsArgs),
    /// Run a plugin command through the normal permission and audit flow
    Run {
        id: String,
        command: String,
        #[arg(long, help = "Use an existing session in the current workspace")]
        session: Option<String>,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        arguments: Vec<String>,
    },
    /// 【插件命令】【直接工具】使用 JSON 参数调用已启用插件的工具
    #[command(
        about = "Call an enabled plugin tool with JSON arguments through normal permissions"
    )]
    Call {
        id: String,
        tool: String,
        #[arg(long, help = "Use an existing session in the current workspace")]
        session: Option<String>,
        #[arg(default_value = "{}")]
        arguments: String,
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
        PluginsCommand::Jobs(args) => return super::plugin_jobs::run(paths, args),
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
        PluginsCommand::Pack { directory, output } => {
            let result = plugins::pack(directory, output.as_deref())?;
            return print_result(
                args.json,
                serde_json::to_value(&result)?,
                &format!(
                    "{}: {}\nSHA-256: {}",
                    t("Packed", "已打包"),
                    result.path.display(),
                    result.sha256,
                ),
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
            allow_http_read_any,
            no_http_read_any,
            allow_http_read_only_post,
            no_http,
            allow_model,
            no_model,
            allow_vision,
            no_vision,
            allow_notifications,
            no_notifications,
            allow_tui_status,
            no_tui_status,
            allow_reply_policy,
            no_reply_policy,
            allow_notify,
            no_notify,
            allow_schedule,
            no_schedule,
            allow_tool,
            no_tools,
            allow_read_path,
            no_file_read,
            allow_remove_path,
            no_file_remove,
            allow_trash_path,
            no_file_trash,
            allow_env,
            no_env,
            allow_process,
            no_processes,
            allow_session_storage,
            no_session_storage,
            allow_plugin_storage,
            no_plugin_storage,
            allow_workspace,
            no_workspace,
            allow_public_downloads,
            no_public_downloads,
            allow_write_path,
            no_file_write,
            allow_image_display,
            no_image_display,
        } => {
            let update = if grant_declared {
                GrantUpdate::Declared
            } else if no_http
                || allow_http_read_any
                || no_http_read_any
                || !allow_http.is_empty()
                || allow_model
                || no_model
                || allow_vision
                || no_vision
                || allow_notifications
                || no_notifications
                || allow_tui_status
                || no_tui_status
                || allow_reply_policy
                || no_reply_policy
                || allow_notify
                || no_notify
                || allow_schedule
                || no_schedule
                || !allow_tool.is_empty()
                || no_tools
                || !allow_read_path.is_empty()
                || no_file_read
                || !allow_remove_path.is_empty()
                || no_file_remove
                || !allow_trash_path.is_empty()
                || no_file_trash
                || !allow_env.is_empty()
                || no_env
                || !allow_process.is_empty()
                || no_processes
                || allow_session_storage
                || no_session_storage
                || allow_plugin_storage
                || no_plugin_storage
                || allow_workspace
                || no_workspace
                || allow_public_downloads
                || no_public_downloads
                || !allow_write_path.is_empty()
                || no_file_write
                || allow_image_display
                || no_image_display
            {
                let change_http = no_http || !allow_http.is_empty();
                GrantUpdate::Changes(GrantChanges {
                    public_downloads: (allow_public_downloads || no_public_downloads)
                        .then_some(allow_public_downloads),
                    write_paths: (no_file_write || !allow_write_path.is_empty())
                        .then(|| allow_write_path.into_iter().collect()),
                    display_images: (allow_image_display || no_image_display)
                        .then_some(allow_image_display),
                    session_storage: (allow_session_storage || no_session_storage)
                        .then_some(allow_session_storage),
                    plugin_storage: (allow_plugin_storage || no_plugin_storage)
                        .then_some(allow_plugin_storage),
                    workspace: (allow_workspace || no_workspace).then_some(allow_workspace),
                    http: change_http.then(|| allow_http.into_iter().collect()),
                    http_read_any: (allow_http_read_any || no_http_read_any || no_http)
                        .then_some(allow_http_read_any),
                    http_read_only_post: change_http
                        .then(|| allow_http_read_only_post.into_iter().collect()),
                    model: (allow_model || no_model).then_some(allow_model),
                    vision: (allow_vision || no_vision).then_some(allow_vision),
                    notifications: (allow_notifications || no_notifications)
                        .then_some(allow_notifications),
                    tui_status: (allow_tui_status || no_tui_status).then_some(allow_tui_status),
                    reply_policy: (allow_reply_policy || no_reply_policy)
                        .then_some(allow_reply_policy),
                    notify: (allow_notify || no_notify).then_some(allow_notify),
                    schedule: (allow_schedule || no_schedule).then_some(allow_schedule),
                    tools: (no_tools || !allow_tool.is_empty())
                        .then(|| allow_tool.into_iter().collect()),
                    read_paths: (no_file_read || !allow_read_path.is_empty())
                        .then(|| allow_read_path.into_iter().collect()),
                    remove_paths: (no_file_remove || !allow_remove_path.is_empty())
                        .then(|| allow_remove_path.into_iter().collect()),
                    trash_paths: (no_file_trash || !allow_trash_path.is_empty())
                        .then(|| allow_trash_path.into_iter().collect()),
                    environment: (no_env || !allow_env.is_empty())
                        .then(|| allow_env.into_iter().collect()),
                    processes: (no_processes || !allow_process.is_empty())
                        .then(|| allow_process.into_iter().collect()),
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
            session,
        } => {
            let mode = mode.unwrap_or_else(|| config.permission.cli_mode().into());
            let output = super::plugin_execute::command(
                &config,
                paths,
                &id,
                &command,
                &arguments.join(" "),
                session.as_deref(),
                mode,
            )
            .await?;
            print_result(args.json, json!({"output": output}), &output)
        }
        PluginsCommand::Call {
            id,
            tool,
            arguments,
            session,
        } => {
            let mode = mode.unwrap_or_else(|| config.permission.cli_mode().into());
            let output = super::plugin_execute::tool(
                &config,
                paths,
                &id,
                &tool,
                &arguments,
                session.as_deref(),
                mode,
            )
            .await?;
            print_result(args.json, json!({"output": output}), &output)
        }
        PluginsCommand::Init { .. }
        | PluginsCommand::Check { .. }
        | PluginsCommand::Pack { .. }
        | PluginsCommand::Jobs(_)
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
