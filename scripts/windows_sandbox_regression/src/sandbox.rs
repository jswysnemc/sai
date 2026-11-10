use serde::{Deserialize, Serialize};
use std::{ffi::OsString, path::PathBuf};
#[derive(Clone, Serialize, Deserialize)]
pub(crate) enum FileAccess {
    ReadOnly,
    WorkspaceWrite,
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct SandboxPolicy {
    pub access: FileAccess,
    pub cwd: PathBuf,
    pub writable_roots: Vec<PathBuf>,
    pub write_protected: Vec<PathBuf>,
    pub deny_read: Vec<PathBuf>,
    pub network: bool,
    pub allowed_domains: Vec<String>,
    pub temp_dir: Option<PathBuf>,
    #[serde(skip)]
    pub env: Vec<(OsString, OsString)>,
    pub scrubbed_env: Vec<String>,
}
pub(crate) fn launch(
    policy: &SandboxPolicy,
    shell: &str,
    script: &str,
) -> anyhow::Result<tokio::process::Command> {
    windows::command(policy, shell, script)
}
#[path = "../../../src/sandbox/windows/mod.rs"]
pub(crate) mod windows;
