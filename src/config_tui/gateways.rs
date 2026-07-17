use crate::config::AppConfig;
use crate::gateways::manager::{
    gateway_runtime_statuses, start_gateway, stop_gateway, GatewayRuntimeStatus, ManagedGateway,
};
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::Result;
use crossterm::event::KeyCode;
use std::future::Future;
use std::io;

use super::form::{parse_bool_field, run_form, Field};
use super::input::read_key;
use super::ui::draw_menu;

/// 编辑渠道接入配置。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `config`: 应用配置
///
/// 返回:
/// - 编辑是否成功
pub(crate) fn edit_gateways(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    config: &mut AppConfig,
) -> Result<()> {
    let mut selected = 0usize;
    let mut status_line = String::new();
    loop {
        let statuses = load_gateway_statuses(paths, config)?;
        let options = vec![
            gateway_status_label(&statuses, ManagedGateway::Qq),
            gateway_status_label(&statuses, ManagedGateway::Weixin),
            t("Edit QQ official bot", "编辑 QQ 官方机器人").to_string(),
            t("Edit Weixin iLink bot", "编辑微信 iLink 机器人").to_string(),
            t("Back", "返回").to_string(),
        ];
        let help = if status_line.trim().is_empty() {
            t(
                "Enter toggles selected gateway, s start, x stop, r refresh",
                "Enter 切换选中网关，s 启动，x 停止，r 刷新",
            )
            .to_string()
        } else {
            status_line.clone()
        };
        draw_menu(
            stdout,
            t(" GATEWAYS ", " 渠道接入 "),
            &options,
            selected,
            &help,
        )?;
        match read_key()? {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Enter => match selected {
                0 => toggle_gateway(paths, config, ManagedGateway::Qq, &mut status_line)?,
                1 => toggle_gateway(paths, config, ManagedGateway::Weixin, &mut status_line)?,
                2 => edit_qq_gateway(stdout, config)?,
                3 => edit_weixin_gateway(stdout, config)?,
                4 => return Ok(()),
                _ => {}
            },
            KeyCode::Char('s') => {
                if let Some(gateway) = selected_gateway(selected) {
                    start_selected_gateway(paths, config, gateway, &mut status_line)?;
                }
            }
            KeyCode::Char('x') => {
                if let Some(gateway) = selected_gateway(selected) {
                    stop_selected_gateway(paths, config, gateway, &mut status_line)?;
                }
            }
            KeyCode::Char('r') => status_line = t("refreshed", "已刷新").to_string(),
            _ => {}
        }
    }
}

/// 加载网关运行状态。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
///
/// 返回:
/// - 网关运行状态列表
fn load_gateway_statuses(
    paths: &SaiPaths,
    config: &AppConfig,
) -> Result<Vec<GatewayRuntimeStatus>> {
    block_on_runtime(gateway_runtime_statuses(paths, config))
}

/// 组装网关状态标签。
///
/// 参数:
/// - `statuses`: 网关状态列表
/// - `gateway`: 网关
///
/// 返回:
/// - 菜单标签
fn gateway_status_label(statuses: &[GatewayRuntimeStatus], gateway: ManagedGateway) -> String {
    let status = statuses
        .iter()
        .find(|status| status.gateway == gateway)
        .cloned();
    let enabled = status
        .as_ref()
        .map(|status| status.enabled)
        .unwrap_or(false);
    let runtime = status
        .as_ref()
        .map(|status| status.status.as_str())
        .unwrap_or("stopped");
    let pid = status
        .as_ref()
        .and_then(|status| status.pid)
        .map(|pid| format!(" pid={pid}"))
        .unwrap_or_default();
    let task = status
        .as_ref()
        .and_then(|status| status.task_id.as_deref())
        .map(|task_id| format!(" task={}", short_task_id(task_id)))
        .unwrap_or_default();
    format!(
        "{} [{}] config={}{}{}",
        gateway.title(),
        runtime,
        if enabled { "enabled" } else { "disabled" },
        pid,
        task
    )
}

/// 返回短任务 ID。
///
/// 参数:
/// - `task_id`: 后台任务 ID
///
/// 返回:
/// - 短 ID
fn short_task_id(task_id: &str) -> String {
    task_id.chars().take(18).collect()
}

/// 返回选中行对应网关。
///
/// 参数:
/// - `selected`: 选中索引
///
/// 返回:
/// - 可选网关
fn selected_gateway(selected: usize) -> Option<ManagedGateway> {
    match selected {
        0 => Some(ManagedGateway::Qq),
        1 => Some(ManagedGateway::Weixin),
        _ => None,
    }
}

/// 根据当前状态启动或停止网关。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `gateway`: 网关
/// - `status_line`: 状态提示文本
///
/// 返回:
/// - 操作是否成功
fn toggle_gateway(
    paths: &SaiPaths,
    config: &AppConfig,
    gateway: ManagedGateway,
    status_line: &mut String,
) -> Result<()> {
    let statuses = load_gateway_statuses(paths, config)?;
    let running = statuses
        .iter()
        .find(|status| status.gateway == gateway)
        .map(|status| status.status == "running")
        .unwrap_or(false);
    if running {
        stop_selected_gateway(paths, config, gateway, status_line)
    } else {
        start_selected_gateway(paths, config, gateway, status_line)
    }
}

/// 启动选中网关。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `gateway`: 网关
/// - `status_line`: 状态提示文本
///
/// 返回:
/// - 操作是否成功
fn start_selected_gateway(
    paths: &SaiPaths,
    config: &AppConfig,
    gateway: ManagedGateway,
    status_line: &mut String,
) -> Result<()> {
    config.save(paths)?;
    block_on_runtime(start_gateway(paths, config, gateway))?;
    *status_line = format!("{}: {}", t("started", "已启动"), gateway.title());
    Ok(())
}

/// 停止选中网关。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `gateway`: 网关
/// - `status_line`: 状态提示文本
///
/// 返回:
/// - 操作是否成功
fn stop_selected_gateway(
    paths: &SaiPaths,
    config: &AppConfig,
    gateway: ManagedGateway,
    status_line: &mut String,
) -> Result<()> {
    let stopped = block_on_runtime(stop_gateway(paths, config, gateway))?;
    *status_line = format!(
        "{}: {} ({stopped})",
        t("stopped", "已停止"),
        gateway.title()
    );
    Ok(())
}

/// 在同步 TUI 中运行异步任务。
///
/// 参数:
/// - `future`: 异步任务
///
/// 返回:
/// - 异步任务结果
fn block_on_runtime<F: Future>(future: F) -> F::Output {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
}

/// 编辑 QQ 官方机器人配置。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `config`: 应用配置
///
/// 返回:
/// - 编辑是否成功
fn edit_qq_gateway(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let qq = &config.gateways.qq;
    let mut fields = vec![
        Field::boolean(t("Enabled", "启用此渠道"), qq.enabled),
        Field::new(
            t(
                "Transport, websocket/webhook",
                "传输模式，websocket/webhook",
            ),
            qq.transport.clone(),
        ),
        Field::new(
            t("Webhook listen address", "Webhook 监听地址"),
            qq.listen.clone(),
        ),
        Field::new(
            t("OpenAPI base URL", "OpenAPI 基础地址"),
            qq.base_url.clone(),
        ),
        Field::new(
            t("Token, AppID:AppSecret", "Token，格式 AppID:AppSecret"),
            qq.token.clone(),
        )
        .secret(),
        Field::new(t("AppID", "AppID"), qq.app_id.clone()),
        Field::new(t("AppSecret", "AppSecret"), qq.client_secret.clone()).secret(),
    ];
    if run_form(
        stdout,
        t(" QQ OFFICIAL BOT ", " QQ 官方机器人 "),
        &mut fields,
    )? {
        config.gateways.qq.enabled = parse_bool_field(&fields[0].value)?;
        config.gateways.qq.transport = fields[1].value.trim().to_string();
        config.gateways.qq.listen = fields[2].value.trim().to_string();
        config.gateways.qq.base_url = fields[3].value.trim().to_string();
        config.gateways.qq.token = fields[4].value.trim().to_string();
        config.gateways.qq.app_id = fields[5].value.trim().to_string();
        config.gateways.qq.client_secret = fields[6].value.trim().to_string();
    }
    Ok(())
}

/// 编辑微信 iLink 机器人配置。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `config`: 应用配置
///
/// 返回:
/// - 编辑是否成功
fn edit_weixin_gateway(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let weixin = &config.gateways.weixin;
    let mut fields = vec![
        Field::boolean(t("Enabled", "启用此渠道"), weixin.enabled),
        Field::new(
            t("iLink base URL", "iLink 基础地址"),
            weixin.base_url.clone(),
        ),
        Field::new(
            t("CDN base URL", "CDN 基础地址"),
            weixin.cdn_base_url.clone(),
        ),
        Field::new(
            t("Login bot type", "登录机器人类型"),
            weixin.bot_type.clone(),
        ),
        Field::new(t("Token", "Token"), weixin.token.clone()).secret(),
        Field::new(t("Saved account", "已保存账号"), weixin.account.clone()).secret(),
        Field::new(t("Bot agent", "Bot Agent"), weixin.bot_agent.clone()),
    ];
    if run_form(
        stdout,
        t(" WEIXIN ILINK BOT ", " 微信 iLink 机器人 "),
        &mut fields,
    )? {
        config.gateways.weixin.enabled = parse_bool_field(&fields[0].value)?;
        config.gateways.weixin.base_url = fields[1].value.trim().to_string();
        config.gateways.weixin.cdn_base_url = fields[2].value.trim().to_string();
        config.gateways.weixin.bot_type = fields[3].value.trim().to_string();
        config.gateways.weixin.token = fields[4].value.trim().to_string();
        config.gateways.weixin.account = fields[5].value.trim().to_string();
        config.gateways.weixin.bot_agent = fields[6].value.trim().to_string();
    }
    Ok(())
}
