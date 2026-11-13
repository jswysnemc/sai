//! 用户审查记录独立于项目定义，文件内容变化后必须重新授权。
use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use std::{collections::BTreeMap, io::Write, path::Path};

/// 【Hooks】【信任读取】参数为用户路径，返回只由用户配置目录保存的内容摘要映射
pub(super) fn load(paths: &SaiPaths) -> Result<BTreeMap<String, String>> {
    let file = paths.config_dir.join("hooks-trust.json");
    if !file.exists() {
        return Ok(BTreeMap::new());
    }
    let text = super::source_io::read(&file, 1_048_576)?;
    anyhow::ensure!(text.len() <= 1_048_576, "hook trust file exceeds 1 MiB");
    serde_json::from_slice(&text).context("invalid hooks-trust.json")
}

/// 【Hooks】【审查授权】参数为用户路径、来源与已显示摘要，返回原子保存结果；内容变化时拒绝授权
pub(crate) fn approve(
    paths: &SaiPaths,
    source: &super::sources::SourcePreview,
    digest: &str,
) -> Result<()> {
    anyhow::ensure!(
        !source.managed,
        "managed hook trust is controlled by the administrator"
    );
    anyhow::ensure!(
        source.error.is_none() && !source.digest.is_empty(),
        "invalid hook source cannot be trusted"
    );
    anyhow::ensure!(
        source.digest == digest,
        "hook source changed; review it again"
    );
    let mut trusted = load(paths)?;
    trusted.insert(source.path.clone(), digest.into());
    std::fs::create_dir_all(&paths.config_dir)?;
    let mut temp = tempfile::NamedTempFile::new_in(&paths.config_dir)?;
    temp.write_all(&serde_json::to_vec_pretty(&trusted)?)?;
    temp.as_file().sync_all()?;
    temp.persist(paths.config_dir.join("hooks-trust.json"))?;
    Ok(())
}

/// 【Hooks】【托管来源】参数为策略文件，返回是否具有管理员所有权且普通用户不能替换
pub(super) fn managed_file(file: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        for item in file.ancestors() {
            let metadata = std::fs::symlink_metadata(item)?;
            anyhow::ensure!(
                !metadata.file_type().is_symlink()
                    && metadata.uid() == 0
                    && metadata.mode() & 0o022 == 0,
                "managed hooks require administrator-owned, non-writable files and directories"
            );
        }
    }
    #[cfg(windows)]
    anyhow::bail!(
        "managed hooks require a verified administrator policy; this platform is not configured"
    );
    Ok(())
}
