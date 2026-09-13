use crate::agent::AgentMode;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::tools::ToolRegistry;
use anyhow::{bail, Context, Result};
use serde_json::json;

/// 【插件命令】【命令调用】把注册命令转换为普通工具，复用统一权限流程
/// @param config 主配置；paths 为应用路径；id 和 command 定位命令；arguments 为文本；session 为可选会话；mode 为权限模式
/// @returns 命令输出，拒绝授权时不执行回调
pub(super) async fn command(
    config: &AppConfig,
    paths: &SaiPaths,
    id: &str,
    command: &str,
    arguments: &str,
    session: Option<&str>,
    mode: AgentMode,
) -> Result<String> {
    let mut registry = registry(config, paths, false)?;
    let tool = registry.plugin_command(id, command)?;
    let name = tool.name.clone();
    registry.register(tool);
    invoke(
        registry,
        paths,
        &name,
        &json!({"arguments":arguments}).to_string(),
        session,
        mode,
    )
    .await
}

/// 【插件命令】【工具调用】直接调用指定包拥有的工具，继续校验原始参数 Schema
/// @param config 主配置；paths 为应用路径；id 为包标识；local 为包内工具名；arguments 为 JSON；session 为可选会话；mode 为权限模式
/// @returns 工具输出；缺少插件、错误归属或参数非法时失败
pub(super) async fn tool(
    config: &AppConfig,
    paths: &SaiPaths,
    id: &str,
    local: &str,
    arguments: &str,
    session: Option<&str>,
    mode: AgentMode,
) -> Result<String> {
    let arguments: serde_json::Value =
        serde_json::from_str(arguments).context("plugin tool arguments must be JSON")?;
    let registry = registry(config, paths, matches!(mode, AgentMode::Plan))?;
    let name = [format!("lua__{id}__{local}"), local.to_string()]
        .into_iter()
        .find(|name| registry.plugin_owner(name) == Some(id))
        .with_context(|| format!("enabled plugin tool not found: {id}/{local}"))?;
    invoke(
        registry,
        paths,
        &name,
        &arguments.to_string(),
        session,
        mode,
    )
    .await
}

/// 【插件命令】【真实注册】构造普通 CLI 工具集合，不初始化模型或 MCP 服务
/// @param config 主配置；paths 为应用路径；readonly 选择工具的只读实现
/// @returns 完整本地工具表；插件加载错误保留诊断
fn registry(config: &AppConfig, paths: &SaiPaths, readonly: bool) -> Result<ToolRegistry> {
    let registry = if readonly {
        crate::tools::readonly_registry(config, paths)
    } else {
        crate::tools::builtin_registry_without_mcp(config, paths)
    };
    if !registry.plugin_diagnostics().is_empty() {
        bail!(
            "plugin loading failed: {}",
            serde_json::to_string(registry.plugin_diagnostics())?
        );
    }
    Ok(registry)
}

/// 【插件命令】【权限执行】共用会话、授权、审计和实际工具调用
/// @param registry 已注册工具表；paths 为应用路径；name 为公开工具名；arguments 为 JSON；session 为目标会话；mode 为权限模式
/// @returns 工具文本输出；用户拒绝时不执行 Lua 回调
async fn invoke(
    mut registry: ToolRegistry,
    paths: &SaiPaths,
    name: &str,
    arguments: &str,
    session: Option<&str>,
    mode: AgentMode,
) -> Result<String> {
    // 1. 【插件命令】【会话权限】直接用户调用使用 CLI 权限模式和独立审计记录
    let workdir = crate::runtime_cwd::current_dir()?;
    let session_id = session.map(str::trim).unwrap_or("plugin-command");
    let storage_session = if let Some(session) = session {
        let (_, directory) =
            crate::state::state_dir_for_workspace_session(paths, &workdir, session)?;
        directory
            .to_str()
            .context("session path must be UTF-8")?
            .to_string()
    } else {
        format!(
            "plugin-command/{}",
            dunce::canonicalize(&workdir)?.display()
        )
    };
    registry.start_plugin_session(session_id)?;
    registry.inherit_plugin_storage_session(&storage_session);
    registry.set_permission_profile(crate::permission::PermissionProfile::new(
        mode.permission_profile_mode(),
        workdir,
        Some(crate::permission::PermissionAuditLog::new(
            paths.data_dir.join("permission-audit-cli.jsonl"),
            session_id,
        )),
    ));
    if registry.requires_permission(name, arguments)? {
        registry.record_permission_requested(name, arguments)?;
        let (request, receiver) =
            crate::permission::request_permission(session_id, name, arguments);
        super::prompt_permission_request(&request)?;
        let decision = receiver.await?;
        let detail = decision.detail().map(str::to_string);
        match decision {
            crate::permission::PermissionDecision::Allow { .. } => {
                registry.record_permission_approved(name, arguments, detail.as_deref())?
            }
            crate::permission::PermissionDecision::Deny { reply } => {
                registry.record_permission_denied(name, arguments, reply.as_deref())?;
                bail!(
                    "{}",
                    reply.unwrap_or_else(|| "plugin invocation denied".to_string())
                );
            }
        }
    }
    // 2. 【插件命令】【执行契约】保留参数校验、能力交集、只读限制和调用预算
    registry.call(name, arguments).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::{self, GrantUpdate};

    /// 【插件命令测试】【独立环境】安装无外部权限的读写工具和文本命令样本
    /// @returns 临时目录、应用路径和缺省配置
    fn fixture() -> (tempfile::TempDir, SaiPaths, AppConfig) {
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(&root.path().join("app"));
        let config = AppConfig::default();
        let source = root.path().join("source");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("sai-plugin.json"), json!({
            "api_version":1, "id":"direct-call", "version":"1.0.0", "name":"direct-call",
            "description":"Direct plugin invocation contract", "entry":"init.lua", "capabilities":{}
        }).to_string()).unwrap();
        std::fs::write(source.join("init.lua"), r#"
            --- 【插件命令测试】【回显】返回已校验工具参数中的文本
            --- @param args table 含 text 字符串的参数
            --- @return string 原始文本
            local function echo(args) return args.text end
            --- 【插件命令测试】【写入探针】标记写入回调确实执行
            --- @return string 固定成功结果
            local function change() return "changed" end
            --- 【插件命令测试】【命令探针】保持用户命令的文本参数契约
            --- @param arguments string 用户输入
            --- @return string 原始输入
            local function command(arguments) return arguments end
            --- 【插件命令测试】【双模式探针】返回宿主提供的实际写入许可
            --- @param args table 空参数
            --- @param ctx table 可信调用上下文
            --- @return string 实际写入许可
            local function optional(args, ctx) return tostring(ctx.allow_writes) end
            sai.register_tool({name="echo", description="Echo validated text", access="read_only",
                parameters={type="object",properties={text={type="string"}},required={"text"},additionalProperties=false},execute=echo})
            sai.register_tool({name="change", description="Write permission probe", access="writes",
                parameters={type="object",additionalProperties=false},execute=change})
            sai.register_tool({name="optional", description="Optional writes probe", access="optional_writes",
                parameters={type="object",additionalProperties=false},execute=optional})
            sai.register_command({name="echo",description="Echo command text",access="read_only",execute=command})
        "#).unwrap();
        plugins::install(&source, &paths, false).unwrap();
        plugins::set_enabled(&config, &paths, "direct-call", true, GrantUpdate::Keep).unwrap();
        (root, paths, config)
    }

    /// 【插件命令测试】【真实参数】直接调用继续校验工具 Schema 并验证包归属
    /// @returns 无；非法 JSON、参数类型和借用原生工具都必须失败
    #[tokio::test]
    async fn direct_tools_validate_arguments_and_ownership() {
        let (_root, paths, config) = fixture();
        assert_eq!(
            tool(
                &config,
                &paths,
                "direct-call",
                "echo",
                r#"{"text":"文本"}"#,
                None,
                AgentMode::Plan
            )
            .await
            .unwrap(),
            "文本"
        );
        assert_eq!(
            tool(
                &config,
                &paths,
                "direct-call",
                "lua__direct-call__echo",
                r#"{"text":"full"}"#,
                None,
                AgentMode::Plan
            )
            .await
            .unwrap(),
            "full"
        );
        for arguments in [
            "invalid",
            "{}",
            r#"{"text":4}"#,
            r#"{"text":"ok","extra":true}"#,
        ] {
            assert!(tool(
                &config,
                &paths,
                "direct-call",
                "echo",
                arguments,
                None,
                AgentMode::Plan
            )
            .await
            .is_err());
        }
        assert!(tool(
            &config,
            &paths,
            "direct-call",
            "read_file",
            "{}",
            None,
            AgentMode::Yolo
        )
        .await
        .is_err());
        plugins::set_enabled(&config, &paths, "direct-call", false, GrantUpdate::Keep).unwrap();
        assert!(tool(
            &config,
            &paths,
            "direct-call",
            "echo",
            r#"{"text":"no"}"#,
            None,
            AgentMode::Plan
        )
        .await
        .is_err());
    }

    /// 【插件命令测试】【共用权限】直接工具遵守只读模式，原命令仍接受文本输入
    /// @returns 无；允许写入时才可执行写入回调
    #[tokio::test]
    async fn direct_tools_and_commands_preserve_permissions() {
        let (_root, paths, config) = fixture();
        for (mode, expected) in [(AgentMode::Plan, "false"), (AgentMode::Yolo, "true")] {
            assert_eq!(
                tool(&config, &paths, "direct-call", "optional", "{}", None, mode)
                    .await
                    .unwrap(),
                expected
            );
        }
        assert!(tool(
            &config,
            &paths,
            "direct-call",
            "change",
            "{}",
            None,
            AgentMode::Plan
        )
        .await
        .is_err());
        assert_eq!(
            tool(
                &config,
                &paths,
                "direct-call",
                "change",
                "{}",
                None,
                AgentMode::Yolo
            )
            .await
            .unwrap(),
            "changed"
        );
        assert_eq!(
            command(
                &config,
                &paths,
                "direct-call",
                "echo",
                "command text",
                None,
                AgentMode::Plan
            )
            .await
            .unwrap(),
            "command text"
        );
    }

    /// 【插件命令测试】【指定会话】命令导入与 Web 查询使用同一可信目录，工作区之间互不覆盖
    /// @returns 无；未指定会话的命令记录与真实会话分离，非法会话不能执行导入
    #[tokio::test]
    async fn explicit_plugin_session_matches_views_and_isolates_workspaces() {
        let (root, paths, config) = fixture();
        let source =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/lua-plugins/todo");
        plugins::install(&source, &paths, false).unwrap();
        plugins::set_enabled(&config, &paths, "todo", true, GrantUpdate::Declared).unwrap();
        let workdir = root.path().join("first");
        std::fs::create_dir(&workdir).unwrap();
        crate::runtime_cwd::scope(workdir.clone(), async {
            let state = crate::state::StateStore::for_workspace_session(&paths, &workdir, "default").unwrap();
            let arguments = json!({"state":{"version":1,"items":[{"id":"old","text":"kept","status":"pending","created_at":"old","updated_at":"old"}],"history":[]}}).to_string();
            command(&config, &paths, "todo", "import", &arguments, Some("default"), AgentMode::Yolo).await.unwrap();
            let view = plugins::todo_view::TodoView::load(&config, &paths).await.unwrap();
            assert_eq!(view.snapshot("default", state.state_dir(), &workdir).await.unwrap().items[0]["text"], "kept");
            let empty = command(&config, &paths, "todo", "snapshot", "", None, AgentMode::Plan).await.unwrap();
            assert_eq!(serde_json::from_str::<serde_json::Value>(&empty).unwrap(), json!({"history":[],"items":[]}));
            assert!(command(&config, &paths, "todo", "import", &arguments, Some("../other"), AgentMode::Yolo).await.is_err());
            let other = root.path().join("second");
            std::fs::create_dir(&other).unwrap();
            let isolated = crate::state::StateStore::for_workspace_session(&paths, &other, "default").unwrap();
            assert!(view.snapshot("default", isolated.state_dir(), &other).await.unwrap().items.is_empty());
        }).await;
    }
}
