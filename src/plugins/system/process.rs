use super::{paths, process_group::ProcessGroup};
use anyhow::{bail, Context, Result};
use sai_plugin_runtime::host::{ProcessOutput, ProcessRequest, SystemContext};
use sai_plugin_runtime::Capabilities;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

/// 【插件系统】【资源次序】取消时先释放进程组，再释放 Child，组长 PID 在终止后代前不会回收。
struct ManagedProcess {
    group: ProcessGroup,
    child: tokio::process::Child,
}

/// 【插件系统】【模板执行】执行固定模板，不经过 shell，不把未授权环境变量传给子进程。
/// @param request 模板和参数；context 为宿主目录与权限；capabilities 为有效授权
/// @returns 有界输出及退出状态；取消时释放进程组和两路管道
pub(in crate::plugins) async fn execute(
    request: ProcessRequest,
    context: SystemContext,
    capabilities: Capabilities,
) -> Result<ProcessOutput> {
    let (program, arguments) = capabilities.system.process_command(
        &request.template,
        &request.parameters,
        context.allow_writes,
    )?;
    let cwd = paths::workdir(&context)?;
    let executable = executable_path(&program, &cwd)?;
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .current_dir(cwd)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for name in &capabilities.system.environment {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    // 【插件系统】【执行归属】1. Unix 建立独立进程组；Windows 在恢复初始线程前加入受管 Job
    let group = ProcessGroup::prepare(&mut command)?;
    let child = command.spawn().context("start plugin process template")?;
    let mut process = ManagedProcess { group, child };
    process.group.attach(&process.child)?;
    let stdout = process
        .child
        .stdout
        .take()
        .context("plugin stdout pipe is unavailable")?;
    let stderr = process
        .child
        .stderr
        .take()
        .context("plugin stderr pipe is unavailable")?;
    // 【插件系统】【并发读取】2. 两路输出并行排空，不启动脱离当前回调的后台读取任务
    let (status, stdout, stderr) = tokio::try_join!(
        process.group.finish(&mut process.child),
        read_bounded(stdout, request.max_stdout_bytes.clamp(1, 2 * 1024 * 1024)),
        read_bounded(stderr, request.max_stderr_bytes.clamp(1, 2 * 1024 * 1024)),
    )?;
    Ok(ProcessOutput {
        status: status.code(),
        stdout: stdout.0,
        stderr: stderr.0,
        timed_out: false,
        stdout_truncated: stdout.1,
        stderr_truncated: stderr.1,
    })
}

/// 【插件系统】【程序定位】使用宿主 PATH 中的绝对目录查找程序，拒绝隐式批处理解释器。
/// @param program 固定程序名称或路径；cwd 为可信工作目录
/// @returns 可执行文件路径
fn executable_path(program: &str, cwd: &Path) -> Result<PathBuf> {
    let path = Path::new(program);
    if path.components().count() > 1 || path.is_absolute() {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            cwd.join(path)
        };
        return check_executable(absolute);
    }
    let search = std::env::var_os("PATH").unwrap_or_default();
    for directory in std::env::split_paths(&search).filter(|directory| directory.is_absolute()) {
        let candidate = directory.join(program);
        #[cfg(windows)]
        let candidate = if candidate.extension().is_none() {
            candidate.with_extension("exe")
        } else {
            candidate
        };
        if executable_file(&candidate) {
            return check_executable(candidate);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("plugin executable was not found: {program}"),
    )
    .into())
}

/// 【插件系统】【程序类型】Windows 批处理文件需要显式解释器模板，不能隐式改变 argv 语义。
/// @param path 待执行的程序路径
/// @returns 已校验路径
fn check_executable(path: PathBuf) -> Result<PathBuf> {
    if !executable_file(&path) {
        bail!("plugin executable is not an executable regular file");
    }
    #[cfg(windows)]
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
        })
    {
        bail!("plugin batch execution requires an explicit interpreter template");
    }
    Ok(path)
}

/// 【插件系统】【执行属性】查找程序时跳过不可执行的同名文件，避免遮蔽后续有效 PATH 条目。
/// @param path 候选程序
/// @returns 是否为当前平台允许执行的普通文件
fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// 【插件系统】【有界管道】持续排空管道，只保存指定字节数，避免子进程阻塞或无限占用内存。
/// @param pipe 异步输出管道；limit 为保存字节数
/// @returns UTF-8 文本与截断标记
async fn read_bounded(mut pipe: impl AsyncRead + Unpin, limit: usize) -> Result<(String, bool)> {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 8192];
    let mut truncated = false;
    loop {
        let read = pipe.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let retained = read.min(limit.saturating_sub(bytes.len()));
        bytes.extend_from_slice(&buffer[..retained]);
        truncated |= retained < read;
    }
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if text.len() > limit {
        let mut end = limit;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        truncated = true;
    }
    Ok((text, truncated))
}
