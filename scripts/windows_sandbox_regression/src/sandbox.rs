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
/// 【Windows沙箱测试】【启动环境】复用生产隔离命令并传入策略提供的环境变量
/// 参数：policy 为隔离策略，shell 为程序路径，script 为命令文本；返回已配置的启动命令
pub(crate) fn launch(
    policy: &SandboxPolicy,
    shell: &str,
    script: &str,
) -> anyhow::Result<tokio::process::Command> {
    let mut command = windows::command(policy, shell, script)?;
    command.envs(policy.env.iter().map(|(key, value)| (key, value)));
    Ok(command)
}
// 【Windows沙箱测试】【生产模块】驱动覆盖受限启动链，主程序使用探测与非受限命令入口
#[expect(
    dead_code,
    reason = "独立驱动只验证受限沙箱，主程序另有探测与非受限命令调用"
)]
#[path = "../../../src/sandbox/windows/mod.rs"]
pub(crate) mod windows;
