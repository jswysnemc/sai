use super::{discover, private::PrivatePluginHost, PluginSource};
use crate::{config::AppConfig, paths::SaiPaths};
use anyhow::{bail, Context, Result};
use sai_plugin_runtime::{InvocationContext, PluginRuntime};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{path::Path, sync::Arc};

/// 【待办界面】【公开快照】保持既有 Web 字段，条目校验与归档规则来自 Lua
#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TodoSnapshot {
    pub items: Vec<Value>,
    pub history: Vec<Value>,
}

/// 【待办界面】【查询实例】同一查询批次复用只读命令实例，不进入 Agent 工具目录
pub(crate) struct TodoView(Option<PluginRuntime>);

impl TodoView {
    /// 【待办界面】【加载当前配置】只执行已启用内置包，文件发现和加载离开异步线程
    /// @param config 当前应用配置；paths 为应用路径
    /// @returns 绑定当前源码和授权的查询实例
    pub(crate) async fn load(config: &AppConfig, paths: &SaiPaths) -> Result<Self> {
        let (config, paths) = (config.clone(), paths.clone());
        let runtime = tokio::task::spawn_blocking(move || -> Result<Option<PluginRuntime>> {
            let found = discover(&config, &paths);
            if let Some(error) = found
                .diagnostics
                .iter()
                .find(|error| matches!(error.source.as_str(), "todo" | "bundled" | "plugins.jsonc"))
            {
                bail!("todo plugin unavailable: {}", error.error);
            }
            let Some(descriptor) = found.plugins.into_iter().find(|item| {
                item.package.manifest.id == "todo" && matches!(item.source, PluginSource::Bundled)
            }) else {
                bail!("bundled todo plugin is missing");
            };
            if !descriptor.setting.enabled {
                return Ok(None);
            }
            let host = Arc::new(PrivatePluginHost::for_descriptor(&paths, &descriptor)?);
            Ok(Some(PluginRuntime::load(
                descriptor.runtime_package(),
                descriptor.settings().clone(),
                descriptor.grants(),
                host,
            )?))
        })
        .await
        .context("load todo view worker")??;
        Ok(Self(runtime))
    }

    /// 【待办界面】【会话投影】通过 Lua 查询当前清单及历史，保留旧 list 的归档语义
    /// @param session 会话标识；directory 为完整状态目录；workdir 为所属工作区
    /// @returns 既有接口快照；禁用时为空，权限或数据错误明确返回
    pub(crate) async fn snapshot(
        &self,
        session: &str,
        directory: &Path,
        workdir: &Path,
    ) -> Result<TodoSnapshot> {
        let Some(runtime) = &self.0 else {
            return Ok(TodoSnapshot::default());
        };
        let output = runtime
            .call_command(
                "snapshot",
                "",
                InvocationContext {
                    session_id: session.into(),
                    storage_session_id: directory.display().to_string(),
                    operation_id: uuid::Uuid::new_v4().to_string(),
                    workdir: workdir.display().to_string(),
                    allow_writes: false,
                    ..Default::default()
                },
            )
            .await?;
        serde_json::from_str(&output).context("decode todo plugin snapshot")
    }
}
