use super::discovery::{diagnostic, discover, PluginDiagnostic};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use sai_plugin_runtime::{Notification, PresentationRuntime, ReplyPresentation, MAX_NOTIFICATIONS};
use serde::Serialize;
use std::time::Duration;
use tokio::time::{timeout_at, Instant};

const MAX_POLICIES: usize = 16;
const PLAN_TIMEOUT: Duration = Duration::from_secs(2);

/// 【插件展示】【计算结果】策略错误独立报告，不改变答复结果，也不触发模型续聊。
#[derive(Default, Serialize)]
pub(crate) struct NotificationPlan {
    pub notifications: Vec<Notification>,
    pub diagnostics: Vec<PluginDiagnostic>,
}

/// 【插件展示】【通知入口】使用当前配置与源码计算一次通知计划，不执行投递。
/// @param config 主配置；paths 为可信插件目录；event 为有限的展示资料
/// @returns 至多八条通知和独立诊断；取消会回收当前 Lua 回调
pub(crate) async fn notification_plan(
    config: &AppConfig,
    paths: &SaiPaths,
    event: ReplyPresentation,
) -> Result<NotificationPlan> {
    event.validate()?;
    let deadline = Instant::now() + PLAN_TIMEOUT;
    let config = config.clone();
    let paths = paths.clone();
    // 1. 【插件展示】【快照发现】文件读取离开异步执行线程；单次计划不复用 Agent 的可变状态
    let found = timeout_at(
        deadline,
        tokio::task::spawn_blocking(move || discover(&config, &paths)),
    )
    .await
    .context("notification discovery timed out")?
    .context("notification discovery worker failed")?;
    let mut plan = NotificationPlan {
        diagnostics: found.diagnostics,
        ..Default::default()
    };
    let policies = found.plugins.into_iter().filter(|descriptor| {
        descriptor.setting.enabled
            && descriptor.capabilities().notifications
            && descriptor.grants().notifications
    });
    for (index, descriptor) in policies.enumerate() {
        if index == MAX_POLICIES {
            plan.diagnostics.push(diagnostic(
                "notifications",
                anyhow::anyhow!("notification policy count exceeds {MAX_POLICIES}"),
            ));
            break;
        }
        let id = descriptor.package.manifest.id.clone();
        // 2. 【插件展示】【能力收窄】每个策略只获得通知授权，宿主 I/O 始终不可用
        let result = timeout_at(deadline, async {
            let runtime = tokio::task::spawn_blocking(move || {
                PresentationRuntime::load(
                    descriptor.runtime_package(),
                    descriptor.settings().clone(),
                    descriptor.grants(),
                )
            })
            .await
            .context("notification loading worker failed")??;
            runtime.reply_end(&event).await
        })
        .await;
        match result {
            Ok(Ok(notifications)) => {
                if plan.notifications.len() + notifications.len() > MAX_NOTIFICATIONS {
                    plan.diagnostics.push(diagnostic(
                        id,
                        anyhow::anyhow!("notification count exceeds {MAX_NOTIFICATIONS}"),
                    ));
                    continue;
                }
                plan.notifications.extend(notifications);
            }
            Ok(Err(error)) => plan.diagnostics.push(diagnostic(id, error)),
            Err(_) => {
                plan.diagnostics.push(diagnostic(
                    id,
                    anyhow::anyhow!("notification plan timed out"),
                ));
                break;
            }
        }
    }
    Ok(plan)
}
