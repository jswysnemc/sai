use super::*;

pub(super) async fn run_config(paths: &SaiPaths, args: ConfigArgs) -> Result<()> {
    match args.command {
        Some(ConfigCommand::Validate) => {
            AppConfig::load(paths)?;
            println!(
                "{}: {}",
                t("config is valid", "配置有效"),
                paths.config_file.display()
            );
            Ok(())
        }
        Some(ConfigCommand::Paths) => {
            paths.print();
            Ok(())
        }
        Some(ConfigCommand::PromptSource(args)) => {
            // 按 Agent 解析后再看：空白 Agent 的效果只有过了档案覆盖才看得出来
            let config = crate::config::apply_agent_override(
                AppConfig::load(paths)?,
                args.agent.as_deref(),
                crate::config::AgentSurface::Cli,
            )?;
            let identity = config.prompt.active_identity.trim();
            println!(
                "base_prompt_source: {}",
                if config.system_prompt.is_some() {
                    "config"
                } else {
                    "built-in"
                }
            );
            println!(
                "active_identity: {}",
                if identity.is_empty() {
                    "(none)"
                } else {
                    identity
                }
            );
            println!(
                "identities_dir: {}",
                config.identities_dir_path(paths).display()
            );
            // 最终发出去的是拼接完各分段的结果，只看基础提示词会漏掉
            // 状态契约、记忆契约与技能目录，"0 提示词"根本验证不了
            let system_prompt =
                crate::agent::system_prompt::build_base_system_prompt(&config, paths, true, None)?;
            println!(
                "system_prompt_first_line: {}",
                system_prompt.lines().next().unwrap_or("(empty)")
            );
            println!("system_prompt_chars: {}", system_prompt.chars().count());
            let tools = crate::tools::builtin_registry_without_mcp(&config, paths);
            // 白名单可以合法地点名会话级工具与插件工具，诊断口径必须覆盖它们：
            // subagent、todo 由 register_interactive_tools 注册，汇率、计算器
            // 一类则取决于插件开关，按当前配置去查都会误报成不存在
            let catalog_config = crate::tools::catalog_config(&config);
            let mut known =
                crate::tools::builtin_registry_without_mcp(&catalog_config, paths);
            crate::tools::register_interactive_tools(
                &mut known,
                &catalog_config,
                paths,
                paths.state_dir.display().to_string(),
                "prompt-source".to_string(),
            );
            let registered: Vec<String> = known
                .definitions()
                .iter()
                .map(|tool| tool.function.name.clone())
                .collect();
            let effective = crate::runner::submission_tools::apply_enabled_tools_filter(
                tools,
                &config,
                crate::runner::SubmissionSource::Repl,
            )?;
            println!("tool_count: {}", effective.definitions().len());
            // 白名单里指向不存在工具的名字会在过滤时被静默丢掉，不点出来
            // 就只表现为「这个 Agent 少了点什么」。MCP 工具按需连接，
            // 此处不建立连接，跳过以免误报
            let stale: Vec<String> = crate::config::unknown_whitelist_tools(&config, &registered)
                .into_iter()
                .filter(|name| !name.starts_with("mcp_"))
                .collect();
            if !stale.is_empty() {
                println!(
                    "{}: {}",
                    t(
                        "unknown_tools_in_whitelist",
                        "白名单中不存在的工具（已被忽略）"
                    ),
                    stale.join(", ")
                );
            }
            Ok(())
        }
        None => crate::config_tui::run(paths),
    }
}
