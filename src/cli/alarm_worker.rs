use super::{AlarmWorkerArgs, SaiPaths};
use anyhow::Result;

/// 【闹钟命令】【旧入口适配】保留旧工作参数和继承的配置目录，业务交给可信 Lua 包。
/// @param paths 当前应用路径；args 为旧父进程传递的工作参数
/// @returns 兼容工作入口结果
pub(super) async fn run_alarm_worker(paths: &SaiPaths, args: AlarmWorkerArgs) -> Result<()> {
    let mut paths = paths.clone();
    paths.state_dir = args.state_dir;
    crate::plugins::run_legacy_alarm_worker(
        &paths,
        &args.id,
        &args.time,
        &args.label,
        args.audio_file.as_deref(),
    )
    .await
}
