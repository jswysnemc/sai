//! 消息渠道网关管理：缓存网关运行状态避免悬停重绘频繁查进程，并在启动网关落盘后同步主会话基线。

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
use super::mouse::{ListMouseState, MenuInput};
use super::ui::{draw_menu_interactive, message};

/// 【渠道接入】【基线同步】网关进程启动并强制写盘后，同步刷新外层配置会话的已保存基线快照。
///
/// 参数:
/// - `config`: 已成功写入磁盘的内存配置
/// - `baseline`: 主菜单持有的序列化基线快照
///
/// 返回:
/// - 无
pub(super) fn sync_saved_baseline(config: &AppConfig, baseline: &mut Option<String>) {
    *baseline = serde_json::to_string(config).ok();
}

/// 【渠道接入】【管理菜单】展示并控制 QQ 与微信网关运行状态，缓存状态结果消除鼠标悬停卡顿。
///
/// 参数:
/// - `stdout`: 终端输出流
/// - `paths`: Sai 路径
/// - `config`: 待修改的应用配置
/// - `baseline`: 主菜单未保存检测基线快照
///
/// 返回:
/// - 菜单交互结果
pub(crate) fn edit_gateways(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    config: &mut AppConfig,
    baseline: &mut Option<String>,
) -> Result<()> {
    let mut selected = 0usize;
    let mut mouse = ListMouseState::default();
    let mut status_line = String::new();
    // 1. 【渠道接入】【状态缓存】仅在进入、启停、修改配置或按 r 刷新时查询进程，鼠标移动重绘直接复用缓存
    let mut statuses = load_gateway_statuses(paths, config)?;
    loop {
        let options = vec![
            gateway_status_label(&statuses, ManagedGateway::Qq),
            gateway_status_label(&statuses, ManagedGateway::Weixin),
            t("Edit QQ official bot", "编辑 QQ 官方机器人").to_string(),
            t("Edit Weixin iLink bot", "编辑微信 iLink 机器人").to_string(),
            t("Back", "返回").to_string(),
        ];
        let details = vec![
            t(
                "Start or stop the QQ official bot process managed by Sai.",
                "启动或停止由 Sai 托管的 QQ 官方机器人进程。",
            )
            .to_string(),
            t(
                "Start or stop the Weixin iLink bot process managed by Sai.",
                "启动或停止由 Sai 托管的微信 iLink 机器人进程。",
            )
            .to_string(),
            t(
                "Edit QQ bot credentials, app id and related gateway settings.",
                "编辑 QQ 机器人凭证、App ID 及相关网关设置。",
            )
            .to_string(),
            t(
                "Edit Weixin iLink credentials and channel options.",
                "编辑微信 iLink 凭证与渠道选项。",
            )
            .to_string(),
            t("Return to the main configuration menu.", "返回配置主菜单。").to_string(),
        ];
        let help = if status_line.trim().is_empty() {
            super::theme::help_line(&[
                ("Enter", t("toggle/edit", "切换/编辑")),
                ("s", t("start", "启动")),
                ("x", t("stop", "停止")),
                ("r", t("refresh", "刷新")),
                ("q", t("back", "返回")),
            ])
        } else {
            status_line.clone()
        };
        let hitbox = draw_menu_interactive(
            stdout,
            t(" GATEWAYS ", " 渠道接入 "),
            &options,
            &details,
            selected,
            &mouse,
            &help,
            "",
        )?;
        let code = match hitbox.read(&mut mouse, selected, false)? {
            MenuInput::Redraw | MenuInput::Paste(_) => continue,
            MenuInput::Select(index) | MenuInput::RightClick(index) => {
                selected = index;
                continue;
            }
            MenuInput::Activate(index) => {
                selected = index;
                KeyCode::Enter
            }
            MenuInput::Action(code) => code,
            MenuInput::Key(key) => key.code,
        };
        match code {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Enter => match selected {
                0 => {
                    toggle_gateway(
                        paths,
                        config,
                        ManagedGateway::Qq,
                        &statuses,
                        &mut status_line,
                        baseline,
                    )?;
                    statuses = load_gateway_statuses(paths, config)?;
                }
                1 => {
                    toggle_gateway(
                        paths,
                        config,
                        ManagedGateway::Weixin,
                        &statuses,
                        &mut status_line,
                        baseline,
                    )?;
                    statuses = load_gateway_statuses(paths, config)?;
                }
                2 => {
                    edit_qq_gateway(stdout, config)?;
                    statuses = load_gateway_statuses(paths, config)?;
                }
                3 => {
                    edit_weixin_gateway(stdout, config)?;
                    statuses = load_gateway_statuses(paths, config)?;
                }
                4 => return Ok(()),
                _ => {}
            },
            KeyCode::Char('s') => {
                if let Some(gateway) = selected_gateway(selected) {
                    start_selected_gateway(paths, config, gateway, &mut status_line, baseline)?;
                    statuses = load_gateway_statuses(paths, config)?;
                }
            }
            KeyCode::Char('x') => {
                if let Some(gateway) = selected_gateway(selected) {
                    stop_selected_gateway(paths, config, gateway, &mut status_line)?;
                    statuses = load_gateway_statuses(paths, config)?;
                }
            }
            KeyCode::Char('r') => {
                statuses = load_gateway_statuses(paths, config)?;
                status_line = t("refreshed", "已刷新").to_string();
            }
            _ => {}
        }
    }
}

/// 加载网关运行状态。
fn load_gateway_statuses(
    paths: &SaiPaths,
    config: &AppConfig,
) -> Result<Vec<GatewayRuntimeStatus>> {
    block_on_runtime(gateway_runtime_statuses(paths, config))
}

/// 组装网关状态标签。
fn gateway_status_label(statuses: &[GatewayRuntimeStatus], gateway: ManagedGateway) -> String {
    let status = statuses
        .iter()
        .find(|status| status.gateway == gateway)
        .cloned();
    let enabled = status
        .as_ref()
        .map(|status| status.enabled)
        .unwrap_or(false);
    let runtime = match status.as_ref().map(|status| status.status.as_str()) {
        Some("running") => t("running", "运行中"),
        _ => t("stopped", "已停止"),
    };
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
        if enabled {
            t("enabled", "已启用")
        } else {
            t("disabled", "已禁用")
        },
        pid,
        task
    )
}

fn short_task_id(task_id: &str) -> String {
    task_id.chars().take(18).collect()
}

fn selected_gateway(selected: usize) -> Option<ManagedGateway> {
    match selected {
        0 => Some(ManagedGateway::Qq),
        1 => Some(ManagedGateway::Weixin),
        _ => None,
    }
}

/// 【渠道接入】【启停切换】依据已缓存状态决定启动或停止目标网关。
fn toggle_gateway(
    paths: &SaiPaths,
    config: &AppConfig,
    gateway: ManagedGateway,
    statuses: &[GatewayRuntimeStatus],
    status_line: &mut String,
    baseline: &mut Option<String>,
) -> Result<()> {
    let running = statuses
        .iter()
        .find(|status| status.gateway == gateway)
        .map(|status| status.status == "running")
        .unwrap_or(false);
    if running {
        stop_selected_gateway(paths, config, gateway, status_line)
    } else {
        start_selected_gateway(paths, config, gateway, status_line, baseline)
    }
}

/// 【渠道接入】【启动并同步基线】校验写盘后启动网关进程，并同步刷新主会话基线。
fn start_selected_gateway(
    paths: &SaiPaths,
    config: &AppConfig,
    gateway: ManagedGateway,
    status_line: &mut String,
    baseline: &mut Option<String>,
) -> Result<()> {
    if let Err(error) = config.validate() {
        *status_line = format!(
            "{}: {error}",
            t("Configuration is invalid", "配置校验未通过")
        );
        return Ok(());
    }
    config.save(paths)?;
    // 1. 【渠道接入】【基线对齐】配置已真实落盘，同步刷新 baseline 消除主菜单虚假的“待保存”状态
    sync_saved_baseline(config, baseline);
    block_on_runtime(start_gateway(paths, config, gateway))?;
    *status_line = format!(
        "{}: {} · {}",
        t("started", "已启动"),
        gateway.title(),
        t("configuration saved to disk", "配置已写入磁盘")
    );
    Ok(())
}

/// 停止选中网关。
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

fn block_on_runtime<F: Future>(future: F) -> F::Output {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
}

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
    loop {
        if !run_form(
            stdout,
            t(" QQ OFFICIAL BOT ", " QQ 官方机器人 "),
            &mut fields,
        )? {
            return Ok(());
        }
        let enabled = match parse_bool_field(&fields[0].value) {
            Ok(enabled) => enabled,
            Err(err) => {
                message(
                    stdout,
                    &format!("{}: {err}", t("Invalid input", "输入无效")),
                )?;
                continue;
            }
        };
        config.gateways.qq.enabled = enabled;
        config.gateways.qq.transport = fields[1].value.trim().to_string();
        config.gateways.qq.listen = fields[2].value.trim().to_string();
        config.gateways.qq.base_url = fields[3].value.trim().to_string();
        config.gateways.qq.token = fields[4].value.trim().to_string();
        config.gateways.qq.app_id = fields[5].value.trim().to_string();
        config.gateways.qq.client_secret = fields[6].value.trim().to_string();
        return Ok(());
    }
}

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
    loop {
        if !run_form(
            stdout,
            t(" WEIXIN ILINK BOT ", " 微信 iLink 机器人 "),
            &mut fields,
        )? {
            return Ok(());
        }
        let enabled = match parse_bool_field(&fields[0].value) {
            Ok(enabled) => enabled,
            Err(err) => {
                message(
                    stdout,
                    &format!("{}: {err}", t("Invalid input", "输入无效")),
                )?;
                continue;
            }
        };
        config.gateways.weixin.enabled = enabled;
        config.gateways.weixin.base_url = fields[1].value.trim().to_string();
        config.gateways.weixin.cdn_base_url = fields[2].value.trim().to_string();
        config.gateways.weixin.bot_type = fields[3].value.trim().to_string();
        config.gateways.weixin.token = fields[4].value.trim().to_string();
        config.gateways.weixin.account = fields[5].value.trim().to_string();
        config.gateways.weixin.bot_agent = fields[6].value.trim().to_string();
        return Ok(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【渠道接入】【基线同步测试】验证网关保存配置后同步更新基线快照，消除主菜单脏标记残留。
    #[test]
    fn gateway_save_syncs_session_baseline() {
        let mut config = AppConfig::default();
        let mut baseline = serde_json::to_string(&config).ok();
        config.gateways.qq.enabled = true;
        assert_ne!(serde_json::to_string(&config).ok(), baseline);

        sync_saved_baseline(&config, &mut baseline);
        assert_eq!(serde_json::to_string(&config).ok(), baseline);
    }
}
