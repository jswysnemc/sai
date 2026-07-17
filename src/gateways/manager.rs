use super::process_control::{
    refresh_gateway_processes, spawn_gateway_process, stop_gateway_process,
};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::state::StateStore;
use anyhow::{Context, Result};
use serde_json::json;

const GATEWAY_QQ: &str = "qq";
const GATEWAY_WEIXIN: &str = "weixin";
const GATEWAY_SCHEDULER: &str = "scheduler";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ManagedGateway {
    Qq,
    Weixin,
}

#[derive(Debug, Clone)]
pub(crate) struct GatewayRuntimeStatus {
    pub(crate) gateway: ManagedGateway,
    pub(crate) enabled: bool,
    pub(crate) task_id: Option<String>,
    pub(crate) status: String,
    pub(crate) pid: Option<u32>,
}

impl ManagedGateway {
    /// 返回网关 ID。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 网关 ID
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Qq => GATEWAY_QQ,
            Self::Weixin => GATEWAY_WEIXIN,
        }
    }

    /// 返回网关显示名称。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 网关显示名称
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Qq => "QQ official bot",
            Self::Weixin => "Weixin iLink bot",
        }
    }

    /// 返回启动命令参数。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 命令参数列表
    fn command_args(self) -> &'static [&'static str] {
        match self {
            Self::Qq => &["gateway", "qq-bot"],
            Self::Weixin => &["gateway", "weixin-server"],
        }
    }
}

/// 返回所有受管理网关。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 网关列表
pub(crate) fn managed_gateways() -> &'static [ManagedGateway] {
    &[ManagedGateway::Qq, ManagedGateway::Weixin]
}

/// 查询所有网关运行状态。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
///
/// 返回:
/// - 网关运行状态列表
pub(crate) async fn gateway_runtime_statuses(
    paths: &SaiPaths,
    config: &AppConfig,
) -> Result<Vec<GatewayRuntimeStatus>> {
    let mut records = refresh_gateway_processes(paths)?;
    let has_running_gateway = managed_gateways().iter().any(|gateway| {
        records
            .iter()
            .any(|record| record.gateway_id == gateway.id() && record.is_running())
    });
    let scheduler_running = records
        .iter()
        .any(|record| record.gateway_id == GATEWAY_SCHEDULER && record.is_running());
    if has_running_gateway && !scheduler_running {
        ensure_scheduler_process(paths, config)?;
        records = refresh_gateway_processes(paths)?;
    }
    Ok(managed_gateways()
        .iter()
        .map(|gateway| {
            let record = records
                .iter()
                .find(|record| record.gateway_id == gateway.id());
            GatewayRuntimeStatus {
                gateway: *gateway,
                enabled: gateway_enabled(config, *gateway),
                task_id: record.map(|record| record.runtime_process_id()),
                status: record
                    .map(|record| record.status.clone())
                    .unwrap_or_else(|| "stopped".to_string()),
                pid: record
                    .filter(|record| record.is_running())
                    .map(|record| record.pid),
            }
        })
        .collect())
}

/// 启动指定网关独立进程。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `gateway`: 网关
///
/// 返回:
/// - JSON 格式启动结果
pub(crate) async fn start_gateway(
    paths: &SaiPaths,
    config: &AppConfig,
    gateway: ManagedGateway,
) -> Result<String> {
    let records = refresh_gateway_processes(paths)?;
    if records
        .iter()
        .any(|record| record.gateway_id == gateway.id() && record.is_running())
    {
        ensure_scheduler_process(paths, config)?;
        return Ok(serde_json::to_string_pretty(&json!({
            "ok": true,
            "already_running": true,
            "gateway": gateway.id(),
        }))?);
    }
    let command = gateway_command(gateway)?;
    let record = spawn_gateway_process(
        paths,
        config,
        gateway.id(),
        &command,
        &gateway_workspace(paths),
    )?;
    ensure_scheduler_process(paths, config)?;
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "gateway": gateway.id(),
        "pid": record.pid,
        "stdout_log": record.stdout_log,
        "stderr_log": record.stderr_log,
    }))?)
}

/// 停止指定网关独立进程。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `gateway`: 网关
///
/// 返回:
/// - 停止的进程数量
pub(crate) async fn stop_gateway(
    paths: &SaiPaths,
    config: &AppConfig,
    gateway: ManagedGateway,
) -> Result<usize> {
    let records = refresh_gateway_processes(paths)?;
    let has_running = records
        .iter()
        .any(|record| record.gateway_id == gateway.id() && record.is_running());
    if has_running {
        // 1. 先按 runtime owner 执行连接关闭策略，再终止独立进程
        let state = StateStore::new(paths)?;
        state.apply_gateway_connection_close_policy(gateway.id())?;
    }
    let stopped = stop_gateway_process(paths, config, gateway.id()).await?;
    let records = refresh_gateway_processes(paths)?;
    let has_running_gateway = managed_gateways().iter().any(|managed| {
        records
            .iter()
            .any(|record| record.gateway_id == managed.id() && record.is_running())
    });
    if !has_running_gateway {
        stop_gateway_process(paths, config, GATEWAY_SCHEDULER).await?;
    }
    Ok(stopped)
}

/// 判断网关是否在配置中启用。
///
/// 参数:
/// - `config`: 应用配置
/// - `gateway`: 网关
///
/// 返回:
/// - 是否启用
fn gateway_enabled(config: &AppConfig, gateway: ManagedGateway) -> bool {
    match gateway {
        ManagedGateway::Qq => config.gateways.qq.enabled,
        ManagedGateway::Weixin => config.gateways.weixin.enabled,
    }
}

/// 组装网关启动命令。
///
/// 参数:
/// - `gateway`: 网关
///
/// 返回:
/// - shell 命令
fn gateway_command(gateway: ManagedGateway) -> Result<String> {
    command_for_args(gateway.command_args())
}

/// 确保网关专属定时调度器独立进程正在运行。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
///
/// 返回:
/// - 调度器进程是否可用
fn ensure_scheduler_process(paths: &SaiPaths, config: &AppConfig) -> Result<()> {
    let records = refresh_gateway_processes(paths)?;
    if records
        .iter()
        .any(|record| record.gateway_id == GATEWAY_SCHEDULER && record.is_running())
    {
        return Ok(());
    }
    let command = command_for_args(&["gateway", "scheduler"])?;
    spawn_gateway_process(
        paths,
        config,
        GATEWAY_SCHEDULER,
        &command,
        &gateway_workspace(paths),
    )?;
    Ok(())
}

/// 组装当前可执行文件及参数对应的 shell 命令。
///
/// 参数:
/// - `args`: 命令参数
///
/// 返回:
/// - shell 命令
fn command_for_args(args: &[&str]) -> Result<String> {
    let exe = std::env::current_exe().context("failed to resolve current executable")?;
    let mut parts = vec![shell_quote(&exe.display().to_string())];
    parts.extend(args.iter().map(|arg| shell_quote(arg)));
    Ok(parts.join(" "))
}

/// 返回网关工作目录。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 工作目录字符串
fn gateway_workspace(paths: &SaiPaths) -> String {
    super::workspace::gateway_workspace_path(paths)
        .display()
        .to_string()
}

/// shell 单引号转义。
///
/// 参数:
/// - `value`: 原始文本
///
/// 返回:
/// - shell 安全文本
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证网关启动命令参数映射稳定。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn gateway_command_args_map_to_cli_subcommands() {
        assert_eq!(ManagedGateway::Qq.command_args(), &["gateway", "qq-bot"]);
        assert_eq!(
            ManagedGateway::Weixin.command_args(),
            &["gateway", "weixin-server"]
        );
    }

    /// 验证 shell 单引号转义。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("a'b"), "'a'\"'\"'b'");
    }
}
