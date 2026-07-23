use super::model::SecretsConfig;
use crate::paths::SaiPaths;
use anyhow::Result;
use std::io::Write;

impl SecretsConfig {
    pub fn load(paths: &SaiPaths) -> Result<Self> {
        if !paths.secrets_file.exists() {
            return Ok(Self::default());
        }
        set_private_permissions(&paths.secrets_file)?;
        let raw = std::fs::read_to_string(&paths.secrets_file)?;
        let stripped = json_comments::StripComments::new(raw.as_bytes());
        Ok(serde_json::from_reader(stripped)?)
    }
}

/// 使用私有权限原子写入凭据文件。
///
/// 参数:
/// - `path`: 目标文件路径
/// - `content`: 待写入字节
///
/// 返回:
/// - 文件替换并确认权限后的结果
pub(super) fn write_private_file(path: &std::path::Path, content: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(parent)?;
    // 1. 已有文件先收紧权限，避免替换准备期间继续暴露旧凭据
    if path.exists() {
        set_private_permissions(path)?;
    }
    // 2. 临时文件默认使用私有权限，写完后原子替换目标
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(content)?;
    set_private_permissions(temp.path())?;
    temp.persist(path)?;
    set_private_permissions(path)?;
    Ok(())
}

#[cfg(unix)]
pub(super) fn set_private_permissions(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o600);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn set_private_permissions(_path: &std::path::Path) -> Result<()> {
    Ok(())
}
