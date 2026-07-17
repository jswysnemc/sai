use super::model::SecretsConfig;
use crate::paths::SaiPaths;
use anyhow::Result;

impl SecretsConfig {
    pub fn load(paths: &SaiPaths) -> Result<Self> {
        if !paths.secrets_file.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&paths.secrets_file)?;
        let stripped = json_comments::StripComments::new(raw.as_bytes());
        Ok(serde_json::from_reader(stripped)?)
    }
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
