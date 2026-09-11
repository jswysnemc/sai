use crate::plugins::system::paths::{expand, resolve_existing_ancestor, workdir};
use anyhow::{bail, Context, Result};
use sai_plugin_runtime::{
    host::{FileRemovalRequest, SystemContext},
    Capabilities,
};
use std::path::{Component, Path, PathBuf};

/// 【插件文件】【删除范围】只规范化父目录，保留末级名称以拒绝链接本身
/// @param request 路径与操作；context 为可信目录和权限；capabilities 为当前有效授权
/// @returns 严格位于某个授权目录内部的绝对目标，不创建目录或文件
pub(super) fn authorize(
    request: &FileRemovalRequest,
    context: &SystemContext,
    capabilities: &Capabilities,
) -> Result<PathBuf> {
    // 1. 【插件文件】【目标语法】明确要求文件名，不把目录末尾的斜杠或点规范成文件删除
    capabilities
        .system
        .check_removal_request(&request.path, request.kind, context.allow_writes)?;
    if request.path.ends_with(['/', '\\'])
        || request.path.split(['/', '\\']).next_back() == Some(".")
    {
        bail!("plugin file removal requires a filename");
    }
    let cwd = workdir(context)?;
    let display = expand(&request.path, &cwd)?;
    validate_components(&display)?;
    let name = display
        .file_name()
        .context("plugin file removal requires a filename")?;
    let parent = display
        .parent()
        .context("plugin file removal requires a parent directory")?;
    let requested = resolve_existing_ancestor(parent)?.join(name);
    // 2. 【插件文件】【独立目录】单文件授权不能代替目录授权，缺失目标仍需通过完整范围检查
    for declared in capabilities.system.removal_paths(request.kind) {
        let root = resolve_existing_ancestor(&expand(declared, &cwd)?)?;
        if requested != root && requested.starts_with(&root) {
            return Ok(requested);
        }
    }
    bail!("plugin file removal is outside the granted paths")
}

/// 【插件文件】【跨平台路径】禁止数据流名称、设备别名及尾部规范化歧义
/// @param path 绝对文件路径
/// @returns 所有普通分量均能作为普通文件名时成功
pub(in crate::plugins) fn validate_components(path: &Path) -> Result<()> {
    for component in path.components() {
        if let Component::Normal(name) = component {
            let name = name.to_string_lossy();
            let base = name
                .split('.')
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            if name.contains(':')
                || name.ends_with(['.', ' '])
                || matches!(
                    base.as_str(),
                    "con" | "prn" | "aux" | "nul" | "conin$" | "conout$"
                )
                || base
                    .strip_prefix("com")
                    .or_else(|| base.strip_prefix("lpt"))
                    .is_some_and(|suffix| {
                        matches!(
                            suffix,
                            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                        )
                    })
            {
                bail!("plugin file operation has a non-portable path component");
            }
        }
    }
    Ok(())
}
