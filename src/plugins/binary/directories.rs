use crate::plugins::{
    file_ops::{cancel, paths::validate_components},
    system::paths::{expand, resolve_existing_ancestor, workdir},
};
use anyhow::{bail, Context, Result};
use sai_plugin_runtime::{host::SystemContext, Capabilities};
use std::sync::{atomic::AtomicBool, Arc};

/// 【插件目录】【受限创建】授权完整目标后逐级创建，不跟随检查后替换的目录链接
/// @param path 目标目录；context 为可信工作目录与权限；capabilities 为输出范围
/// @returns 已创建或存在的规范目录路径
pub(in crate::plugins) async fn create(
    path: String,
    context: SystemContext,
    capabilities: Capabilities,
) -> Result<String> {
    capabilities
        .binary
        .check_write(&path, context.allow_writes)?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let _cancel = cancel::CancelOnDrop(cancelled.clone());
    tokio::task::spawn_blocking(move || {
        let cwd = workdir(&context)?;
        let requested = expand(&path, &cwd)?;
        validate_components(&requested)?;
        let canonical = resolve_existing_ancestor(&requested)?;
        for declared in &capabilities.binary.write_paths {
            let root = resolve_existing_ancestor(&expand(declared, &cwd)?)?;
            if canonical.starts_with(root) {
                cancel::check_cancelled(&cancelled)?;
                super::paths::create_anchor(&canonical)?;
                cancel::check_cancelled(&cancelled)?;
                return Ok(canonical
                    .to_str()
                    .context("plugin directory must be UTF-8")?
                    .to_string());
            }
        }
        bail!("plugin directory is outside the granted write paths")
    })
    .await
    .context("plugin directory worker stopped")?
}
