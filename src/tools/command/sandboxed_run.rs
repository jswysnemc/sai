use super::process::{run_shell_command, run_shell_command_with_progress};
use crate::sandbox::{FileAccess, SandboxPolicy};
use crate::tools::ToolProgress;
use anyhow::Result;
use serde_json::Value;
use std::path::Path;

/// 【沙箱命令】【前台执行】在工作区写入沙箱中执行命令，失败时附带拦截说明。
///
/// 参数:
/// - `command`: Shell 命令文本
/// - `cwd`: 可选工作目录；不在工作区内时回退工作区
/// - `wait_seconds`: 超时秒数
/// - `shell`: 配置指定的 Shell
/// - `progress`: 工具进度通道
///
/// 返回:
/// - JSON 格式命令结果
pub(super) async fn run_workspace_sandboxed(
    command: &str,
    cwd: Option<&Path>,
    wait_seconds: u64,
    shell: &str,
    progress: ToolProgress,
) -> Result<String> {
    let policy = SandboxPolicy::for_current_workspace(FileAccess::WorkspaceWrite, cwd)?;
    let output =
        run_shell_command_with_progress(command, wait_seconds, shell, Some(&policy), progress)
            .await?;
    annotated_output(output, &policy)
}

/// 【沙箱命令】【计划模式】后端可用时在只读沙箱中执行检查命令，否则保持原有直接执行。
///
/// 参数:
/// - `command`: Shell 命令文本
/// - `timeout`: 超时秒数
/// - `shell`: 配置指定的 Shell
///
/// 返回:
/// - JSON 格式命令结果
pub(super) async fn run_plan_command(command: &str, timeout: u64, shell: &str) -> Result<String> {
    // 1. 未启用或本机无后端时退回命令拒绝名单，不因缺少 bwrap 让计划模式无法检查代码
    if !crate::sandbox::sandbox_requested() || !crate::sandbox::backend_availability().available {
        let output = run_shell_command(command, timeout, shell, None).await?;
        return super::run::foreground_output(output);
    }
    // 2. 只读沙箱兜住拒绝名单漏掉的写入，例如 python -c、find -delete
    let policy = SandboxPolicy::for_current_workspace(FileAccess::ReadOnly, None)?;
    let output = run_shell_command(command, timeout, shell, Some(&policy)).await?;
    annotated_output(output, &policy)
}

/// 序列化命令结果；沙箱拦截导致失败时写入 `sandbox_denial`。
///
/// 参数:
/// - `output`: 进程输出
/// - `policy`: 本次使用的沙箱策略
///
/// 返回:
/// - JSON 格式命令结果
fn annotated_output(output: std::process::Output, policy: &SandboxPolicy) -> Result<String> {
    let failed = !output.status.success();
    let denial = failed
        .then(|| {
            crate::sandbox::detect_denial(
                &String::from_utf8_lossy(&output.stdout),
                &String::from_utf8_lossy(&output.stderr),
                policy.network,
            )
        })
        .flatten();
    let text = super::run::foreground_output(output)?;
    let Some((kind, evidence)) = denial else {
        return Ok(text);
    };
    let mut value: Value = serde_json::from_str(&text)?;
    value["sandbox_denial"] =
        crate::sandbox::denial_report(kind, &evidence, policy.access == FileAccess::ReadOnly);
    Ok(serde_json::to_string_pretty(&value)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造测试策略。
    fn policy(access: FileAccess) -> SandboxPolicy {
        let root = std::env::temp_dir();
        SandboxPolicy::resolve(
            &crate::sandbox::SandboxSettings::default(),
            access,
            &root,
            None,
        )
    }

    /// 生成指定退出码与输出的进程结果。
    fn output(code: i32, stderr: &str) -> std::process::Output {
        #[cfg(unix)]
        let status = std::os::unix::process::ExitStatusExt::from_raw(code << 8);
        #[cfg(windows)]
        let status = std::os::windows::process::ExitStatusExt::from_raw(code as u32);
        std::process::Output {
            status,
            stdout: Vec::new(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    /// 验证失败且命中拦截特征时附带说明，成功或普通失败保持原样。
    #[test]
    fn annotates_only_sandbox_failures() {
        let denied = annotated_output(
            output(1, "touch: cannot touch '/x': Read-only file system"),
            &policy(FileAccess::WorkspaceWrite),
        )
        .unwrap();
        let value: Value = serde_json::from_str(&denied).unwrap();
        assert_eq!(value["sandbox_denial"]["kind"], "filesystem");
        assert_eq!(value["success"], false);

        let plain =
            annotated_output(output(1, "assertion failed"), &policy(FileAccess::ReadOnly)).unwrap();
        assert!(!plain.contains("sandbox_denial"));
        let ok = annotated_output(
            output(0, "Read-only file system"),
            &policy(FileAccess::ReadOnly),
        )
        .unwrap();
        assert!(!ok.contains("sandbox_denial"));
    }
}
