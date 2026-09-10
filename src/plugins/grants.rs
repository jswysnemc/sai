use anyhow::{bail, Result};
use sai_plugin_runtime::{BinaryCapabilities, Capabilities, SystemCapabilities};
use std::collections::BTreeSet;

/// 【插件】【授权更新】全量替换与按能力修改分开，撤销网络不会隐式改变模型或工具授权。
pub(crate) enum GrantUpdate {
    Keep,
    Declared,
    Changes(GrantChanges),
}

/// 【插件】【分项授权】未指定的能力保持原值，空集合或 false 明确撤销授权。
#[derive(Default)]
pub(crate) struct GrantChanges {
    pub http: Option<BTreeSet<String>>,
    pub http_read_only_post: Option<BTreeSet<String>>,
    pub model: Option<bool>,
    pub vision: Option<bool>,
    pub notifications: Option<bool>,
    pub tools: Option<BTreeSet<String>>,
    pub read_paths: Option<BTreeSet<String>>,
    pub environment: Option<BTreeSet<String>>,
    pub processes: Option<BTreeSet<String>>,
    pub session_storage: Option<bool>,
    pub workspace: Option<bool>,
    pub public_downloads: Option<bool>,
    pub write_paths: Option<BTreeSet<String>>,
    pub display_images: Option<bool>,
}

impl GrantUpdate {
    /// 【插件】【授权合并】在保存前验证完整交集，不允许授权清单之外的能力。
    /// @param current 当前有效授权；declared 为固定清单声明
    /// @returns 更新后的授权；Keep 返回 None 以保留缺省授权语义
    pub(super) fn resolve(
        self,
        current: Capabilities,
        declared: &Capabilities,
    ) -> Result<Option<Capabilities>> {
        // 【插件】【过期授权】1. 模板或来源变化后，分项修改只保留仍然有效的授权
        let current = current.intersection(declared);
        let grants = match self {
            Self::Keep => return Ok(None),
            Self::Declared => declared.clone(),
            Self::Changes(changes) => Capabilities {
                http: changes.http.unwrap_or(current.http),
                http_read_only_post: changes
                    .http_read_only_post
                    .unwrap_or(current.http_read_only_post),
                model: changes.model.unwrap_or(current.model),
                vision: changes.vision.unwrap_or(current.vision),
                notifications: changes.notifications.unwrap_or(current.notifications),
                tools: changes.tools.unwrap_or(current.tools),
                binary: BinaryCapabilities {
                    public_downloads: changes
                        .public_downloads
                        .unwrap_or(current.binary.public_downloads),
                    write_paths: changes.write_paths.unwrap_or(current.binary.write_paths),
                    display_images: changes
                        .display_images
                        .unwrap_or(current.binary.display_images),
                },
                system: SystemCapabilities {
                    session_storage: changes
                        .session_storage
                        .unwrap_or(current.system.session_storage),
                    workspace: changes.workspace.unwrap_or(current.system.workspace),
                    read_paths: changes.read_paths.unwrap_or(current.system.read_paths),
                    environment: changes.environment.unwrap_or(current.system.environment),
                    processes: match changes.processes {
                        None => current.system.processes,
                        Some(names) => names
                            .into_iter()
                            .map(|name| {
                                let template =
                                    declared.system.processes.get(&name).ok_or_else(|| {
                                        anyhow::anyhow!("process template is not declared: {name}")
                                    })?;
                                Ok((name, template.clone()))
                            })
                            .collect::<Result<_>>()?,
                    },
                },
            },
        };
        grants.validate()?;
        if !grants.is_subset(declared) {
            bail!("plugin grants must be declared in the plugin manifest");
        }
        Ok(Some(grants))
    }
}
