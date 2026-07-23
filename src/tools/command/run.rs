use super::background_tasks::{spawn_managed_task, BackgroundRuntimeOwner};
use super::process::{process_exists, run_shell_command};
#[cfg(test)]
use super::process::terminate_process;
use super::progress::{encode_command_output, CommandOutputStream};
use super::store::{BackgroundCommandStore, BackgroundCommandTask};
use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use crate::tools::{ToolProgress, ToolRegistry, ToolSpec};
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const MAX_COMMAND_OUTPUT_CHARS: usize = 20_000;
const MANAGED_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// 注册可写命令执行工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `config`: 应用配置
/// - `paths`: Sai 路径
/// - `allow_command_execution`: 是否允许执行命令
/// - `runtime_owner`: 可选后台任务运行时 owner
pub(crate) fn register(
    registry: &mut ToolRegistry,
    config: &AppConfig,
    paths: &SaiPaths,
    allow_command_execution: bool,
    runtime_owner: Option<BackgroundRuntimeOwner>,
) {
    let shell = config.tools.command_shell.clone();
    let config = config.clone();
    let paths = paths.clone();
    registry.register(ToolSpec::new_with_progress(
        "run_command",
        t(
            "Run workspace shell commands for builds, tests, validation, export, inspection, and other command-line programs. Prefer edit_file with a Codex-style patch for source text edits, but shell redirection, tee, and heredocs are allowed when useful. A program's own -o/--output option may create build artifacts. Wait up to timeout_seconds (default 30, max 120). Use timeout_seconds=0 to start as a background task immediately. If the command is still running when the wait ends, it is promoted to a background task and task_id is returned; manage it with background_command action=list/output/stop/cleanup.",
            "运行构建、测试、校验、导出、检查及其他命令行程序。源码文本修改优先使用 edit_file 的 Codex 补丁，但 shell 重定向、tee、heredoc 也允许使用。程序自身通过 -o/--output 生成构建产物属于允许用途。等待时间由 timeout_seconds 控制（默认 30，最大 120）。timeout_seconds=0 表示立即作为后台任务启动。若等待结束时命令仍在运行，会提升为后台任务并返回 task_id；随后用 background_command 的 list/output/stop/cleanup 管理。",
        ),
        json!({
            "type":"object",
            "properties":{
                "command":{
                    "type":"string",
                    "description": t(
                        "Complete shell command string. Put pipelines and conditionals in this single field.",
                        "完整 Shell 命令字符串。管道和条件语句都放在此字段中。"
                    )
                },
                "timeout_seconds":{
                    "type":"integer",
                    "minimum":0,
                    "maximum":120,
                    "description": t(
                        "Seconds to wait for completion. 0 starts as background immediately. Defaults to 30. On timeout the process continues as a background task and task_id is returned.",
                        "等待完成的秒数。0 表示立即以后台任务启动，默认 30。超时后进程继续作为后台任务运行并返回 task_id。"
                    )
                },
                "cwd":{
                    "type":"string",
                    "description": t(
                        "Optional working directory. Defaults to current workspace.",
                        "可选工作目录，默认当前工作区。"
                    )
                },
                "label":{
                    "type":"string",
                    "description": t(
                        "Optional human-readable label when the command is promoted or started as a background task.",
                        "命令提升或直接作为后台任务启动时的可选人类可读标签。"
                    )
                },
                "sandbox_permissions":{
                    "type":"string",
                    "enum":["use_default","require_escalated"],
                    "description":t(
                        "Use require_escalated when the command needs network access or must read or write outside the workspace sandbox.",
                        "命令需要网络访问或必须读写工作区沙箱外部时使用 require_escalated。"
                    )
                },
                "justification":{
                    "type":"string",
                    "description":t(
                        "Short reason shown with an elevated permission request.",
                        "提升权限请求中展示的简短原因。"
                    )
                }
            },
            "required":["command"],
            "additionalProperties":false
        }),
        move |args, progress| {
            let shell = shell.clone();
            let config = config.clone();
            let paths = paths.clone();
            let runtime_owner = runtime_owner.clone();
            async move {
                run_command(
                    args,
                    allow_command_execution,
                    shell,
                    progress,
                    &config,
                    &paths,
                    runtime_owner,
                )
                .await
            }
        },
    ).writes());
}

/// 注册只读命令执行工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `config`: 应用配置
pub(crate) fn register_readonly(registry: &mut ToolRegistry, config: &AppConfig) {
    let shell = config.tools.command_shell.clone();
    registry.register(ToolSpec::new(
        "run_command",
        t(
            "Run an explicitly read-only shell command for inspection. Mutating commands are blocked in plan mode. Timeouts fail instead of promoting to background tasks.",
            "运行明确只读的 shell 命令用于检查。计划模式会阻止修改性命令。超时会失败，不会提升为后台任务。",
        ),
        json!({
            "type":"object",
            "properties":{
                "command":{
                    "type":"string",
                    "description": t("Read-only command to run.", "要运行的只读命令。")
                },
                "timeout_seconds":{
                    "type":"integer",
                    "minimum":1,
                    "maximum":120,
                    "description": t("Optional timeout in seconds. Defaults to 30.", "可选超时时间，单位秒，默认 30。")
                }
            },
            "required":["command"],
            "additionalProperties":false
        }),
        move |args| {
            let shell = shell.clone();
            async move { run_readonly_command(args, shell).await }
        },
    ));
}

/// 执行普通 shell 命令。
///
/// 参数:
/// - `args`: 工具参数
/// - `allowed`: 是否允许命令执行
/// - `shell`: 配置指定的 shell，空值表示使用用户环境
/// - `progress`: 工具进度通道
/// - `config`: 应用配置
/// - `paths`: Sai 路径
/// - `runtime_owner`: 可选后台任务运行时 owner
///
/// 返回:
/// - JSON 格式命令结果；超时提升时返回后台任务信息
async fn run_command(
    args: Value,
    allowed: bool,
    shell: String,
    progress: ToolProgress,
    config: &AppConfig,
    paths: &SaiPaths,
    runtime_owner: Option<BackgroundRuntimeOwner>,
) -> Result<String> {
    if !allowed {
        bail!("{}", t("command execution is disabled; set skills.allow_command_execution=true in config.jsonc to enable run_command", "命令执行已禁用；请在 config.jsonc 中设置 skills.allow_command_execution=true 以启用 run_command"));
    }
    let command = required(&args, "command")?;
    let wait_seconds = foreground_wait_seconds(&args);
    let sandboxed = args
        .get("_sai_sandbox")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    // 1. 沙箱命令仍走前台管道执行，超时直接失败，不提升后台
    if sandboxed {
        if wait_seconds == 0 {
            bail!(
                "{}",
                t(
                    "sandboxed run_command cannot start as background; use timeout_seconds >= 1",
                    "沙箱 run_command 不能立即后台启动；请使用 timeout_seconds >= 1"
                )
            );
        }
        let output =
            super::process::run_shell_command_with_progress(&command, wait_seconds, shell.as_str(), true, progress)
                .await?;
        return foreground_output(output);
    }

    // 2. 后台命令未启用时保持同步管道执行
    if !config.tools.background_commands_enabled {
        if wait_seconds == 0 {
            bail!(
                "{}",
                t(
                    "background commands are disabled; timeout_seconds=0 is unavailable",
                    "后台命令已禁用；无法使用 timeout_seconds=0"
                )
            );
        }
        let output = super::process::run_shell_command_with_progress(
            &command,
            wait_seconds,
            shell.as_str(),
            false,
            progress,
        )
        .await?;
        return foreground_output(output);
    }

    // 3. 托管 spawn：超时不杀进程，返回后台 task_id
    let mut managed_args = args.clone();
    if managed_args.get("label").and_then(Value::as_str).is_none() {
        managed_args["label"] = json!(command_label_from_text(&command));
    }
    // 后台任务寿命使用配置默认值（默认 0 表示不自动超时），与前台等待秒数分离
    managed_args["timeout_seconds"] = json!(config.tools.background_command_timeout_seconds);

    let task = spawn_managed_task(managed_args, config, paths, true, runtime_owner)?;
    if wait_seconds == 0 {
        return background_result(&task, 0, "", "");
    }
    wait_managed_task(task, wait_seconds, config, paths, progress).await
}

/// 等待托管任务完成或超时提升。
///
/// 参数:
/// - `task`: 已启动的后台任务
/// - `wait_seconds`: 前台等待秒数
/// - `config`: 应用配置
/// - `paths`: Sai 路径
/// - `progress`: 工具进度通道
///
/// 返回:
/// - 前台完成结果或后台提升结果
async fn wait_managed_task(
    task: BackgroundCommandTask,
    wait_seconds: u64,
    config: &AppConfig,
    paths: &SaiPaths,
    progress: ToolProgress,
) -> Result<String> {
    let deadline = Instant::now() + Duration::from_secs(wait_seconds.max(1));
    let mut stdout_offset = 0u64;
    let mut stderr_offset = 0u64;
    loop {
        // 1. 推送新增日志到进度通道
        stream_log_progress(
            &task.stdout_log,
            &mut stdout_offset,
            CommandOutputStream::Stdout,
            &progress,
        )?;
        stream_log_progress(
            &task.stderr_log,
            &mut stderr_offset,
            CommandOutputStream::Stderr,
            &progress,
        )?;

        // 2. 进程已退出：读取完整日志并返回前台结果
        if !process_exists(task.pid) {
            let store = BackgroundCommandStore::new(paths.state_dir.clone());
            let mut tasks = store.load()?;
            if let Some(existing) = tasks.iter_mut().find(|item| item.id == task.id) {
                if existing.status == "running" {
                    existing.status = "exited".to_string();
                    existing.updated_at = super::store::unix_seconds();
                    store.save(&tasks)?;
                }
            }
            let stdout = read_log_text(&task.stdout_log)?;
            let stderr = read_log_text(&task.stderr_log)?;
            return Ok(serde_json::to_string_pretty(&json!({
                "mode": "foreground",
                "success": true,
                "exit_code": null,
                "stdout": clip_output(&stdout),
                "stderr": clip_output(&stderr),
                "task_id": task.id,
                "note": "Process finished; exact exit code is unavailable for managed wait mode.",
            }))?);
        }

        // 3. 等待时间耗尽：保留进程并返回后台任务
        if Instant::now() >= deadline {
            let stdout = read_log_text(&task.stdout_log)?;
            let stderr = read_log_text(&task.stderr_log)?;
            return background_result(&task, wait_seconds, &stdout, &stderr);
        }

        tokio::time::sleep(MANAGED_POLL_INTERVAL).await;
        let _ = config;
    }
}

/// 将日志新增内容编码为命令输出进度。
///
/// 参数:
/// - `path`: 日志路径
/// - `offset`: 已读取字节偏移
/// - `stream`: 输出流类型
/// - `progress`: 工具进度通道
///
/// 返回:
/// - 读取是否成功
fn stream_log_progress(
    path: &str,
    offset: &mut u64,
    stream: CommandOutputStream,
    progress: &ToolProgress,
) -> Result<()> {
    let path = std::path::Path::new(path);
    if !path.exists() {
        return Ok(());
    }
    let metadata = std::fs::metadata(path)?;
    let len = metadata.len();
    if len <= *offset {
        return Ok(());
    }
    let mut file = std::fs::File::open(path)?;
    use std::io::{Read, Seek, SeekFrom};
    file.seek(SeekFrom::Start(*offset))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    *offset = len;
    if !bytes.is_empty() {
        progress.report(encode_command_output(stream, &bytes));
    }
    Ok(())
}

/// 生成后台提升结果。
///
/// 参数:
/// - `task`: 后台任务
/// - `waited_seconds`: 已等待秒数
/// - `stdout`: 已产生的标准输出
/// - `stderr`: 已产生的标准错误
///
/// 返回:
/// - JSON 字符串
fn background_result(
    task: &BackgroundCommandTask,
    waited_seconds: u64,
    stdout: &str,
    stderr: &str,
) -> Result<String> {
    Ok(serde_json::to_string_pretty(&json!({
        "mode": "background",
        "ok": true,
        "promoted": waited_seconds > 0,
        "reason": if waited_seconds == 0 {
            "started_as_background"
        } else {
            "timed_out_waiting"
        },
        "waited_seconds": waited_seconds,
        "task_id": task.id,
        "task": task,
        "partial_stdout": clip_output(stdout),
        "partial_stderr": clip_output(stderr),
        "note": "Use background_command with action=list, action=output, action=stop, or action=cleanup to manage this task."
    }))?)
}

/// 将同步管道输出格式化为前台结果。
///
/// 参数:
/// - `output`: 进程输出
///
/// 返回:
/// - JSON 字符串
fn foreground_output(output: std::process::Output) -> Result<String> {
    let stdout = clip_output(&String::from_utf8_lossy(&output.stdout));
    let stderr = clip_output(&String::from_utf8_lossy(&output.stderr));
    Ok(serde_json::to_string_pretty(&json!({
        "mode": "foreground",
        "success": output.status.success(),
        "exit_code": output.status.code(),
        "stdout": stdout,
        "stderr": stderr
    }))?)
}

/// 执行只读 shell 命令。
///
/// 参数:
/// - `args`: 工具参数
/// - `shell`: 配置指定的 shell，空值表示使用用户环境
///
/// 返回:
/// - JSON 格式命令结果
async fn run_readonly_command(args: Value, shell: String) -> Result<String> {
    let command = required(&args, "command")?;
    ensure_readonly_command(&command)?;
    let timeout = readonly_timeout(&args);
    let output = run_shell_command(&command, timeout, shell.as_str(), false).await?;
    foreground_output(output)
}

/// 读取前台等待超时参数。
///
/// 参数:
/// - `args`: 工具参数
///
/// 返回:
/// - 等待秒数，0 表示立即后台
fn foreground_wait_seconds(args: &Value) -> u64 {
    args.get("timeout_seconds")
        .and_then(Value::as_u64)
        .unwrap_or(30)
        .min(120)
}

/// 读取只读命令超时参数。
///
/// 参数:
/// - `args`: 工具参数
///
/// 返回:
/// - 超时秒数
fn readonly_timeout(args: &Value) -> u64 {
    args.get("timeout_seconds")
        .and_then(Value::as_u64)
        .unwrap_or(30)
        .clamp(1, 120)
}

/// 从命令文本生成简短标签。
///
/// 参数:
/// - `command`: shell 命令
///
/// 返回:
/// - 标签
fn command_label_from_text(command: &str) -> String {
    command
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("command")
        .chars()
        .take(48)
        .collect()
}

/// 读取日志全文。
///
/// 参数:
/// - `path`: 日志路径
///
/// 返回:
/// - 日志文本
fn read_log_text(path: &str) -> Result<String> {
    let path = std::path::Path::new(path);
    if !path.exists() {
        return Ok(String::new());
    }
    Ok(std::fs::read_to_string(path)?)
}

/// 校验计划模式只读命令。
///
/// 参数:
/// - `command`: shell 命令文本
///
/// 返回:
/// - 命令是否允许
fn ensure_readonly_command(command: &str) -> Result<()> {
    let lower = command.to_ascii_lowercase();
    let forbidden = [
        ">",
        ">>",
        " 2>",
        "tee ",
        "tee -",
        "rm ",
        "mv ",
        "cp ",
        "mkdir ",
        "rmdir ",
        "touch ",
        "chmod ",
        "chown ",
        "chgrp ",
        "ln ",
        "truncate ",
        "dd ",
        "mkfs",
        "mount ",
        "umount ",
        "systemctl ",
        "service ",
        "kill ",
        "pkill ",
        "reboot",
        "shutdown",
        "poweroff",
        "pacman -s",
        "pacman -r",
        "pacman -u",
        "paru -s",
        "yay -s",
        "apt install",
        "apt remove",
        "apt update",
        "dnf install",
        "brew install",
        "sed -i",
        "git add",
        "git commit",
        "git push",
        "git reset",
        "git checkout",
        "cargo build",
        "cargo test",
        "make ",
        "npm install",
        "pnpm install",
        "yarn install",
        "remove-item",
        "ri ",
        " ri ",
        "del ",
        "erase ",
        "copy ",
        "move ",
        "ren ",
        "rename ",
        "move-item",
        "copy-item",
        "new-item",
        "rename-item",
        "set-content",
        "add-content",
        "clear-content",
        "out-file",
        "set-acl",
        "start-process",
        "stop-process",
        "taskkill",
        "winget install",
        "winget uninstall",
        "scoop install",
        "choco install",
    ];
    if forbidden.iter().any(|needle| lower.contains(needle)) {
        bail!(
            "{}",
            t(
                "Plan mode only allows read-only inspection commands",
                "计划模式只允许只读检查命令"
            )
        );
    }
    Ok(())
}

/// 截断命令输出。
///
/// 参数:
/// - `value`: 原始输出
///
/// 返回:
/// - 截断后的输出
fn clip_output(value: &str) -> String {
    let value = value.trim();
    if value.chars().count() <= MAX_COMMAND_OUTPUT_CHARS {
        value.to_string()
    } else {
        format!(
            "{}\n...[{} {MAX_COMMAND_OUTPUT_CHARS} {}]",
            value
                .chars()
                .take(MAX_COMMAND_OUTPUT_CHARS)
                .collect::<String>(),
            t("truncated to", "已截断到"),
            t("chars", "字符")
        )
    }
}

/// 读取必填字符串参数。
///
/// 参数:
/// - `args`: 工具参数
/// - `key`: 参数名
///
/// 返回:
/// - 参数值
fn required(args: &Value, key: &str) -> Result<String> {
    let value = args
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if value.is_empty() {
        bail!("{}: {key}", t("required argument missing", "缺少必需参数"))
    } else {
        Ok(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::SaiPaths;
    use std::path::PathBuf;

    fn test_paths(state_dir: PathBuf) -> SaiPaths {
        SaiPaths {
            config_dir: PathBuf::new(),
            config_file: PathBuf::new(),
            secrets_file: PathBuf::new(),
            skills_dir: PathBuf::new(),
            data_dir: PathBuf::new(),
            cache_dir: PathBuf::new(),
            state_dir,
            pictures_dir: PathBuf::new(),
            fish_hook_file: PathBuf::new(),
            bash_hook_file: PathBuf::new(),
            zsh_hook_file: PathBuf::new(),
            powershell_hook_file: PathBuf::new(),
        }
    }

    #[test]
    fn readonly_command_allows_inspection() {
        assert!(ensure_readonly_command("git status --short").is_ok());
        assert!(ensure_readonly_command("pacman -Q sai").is_ok());
    }

    #[test]
    fn readonly_command_blocks_mutation() {
        assert!(ensure_readonly_command("rm file").is_err());
        assert!(ensure_readonly_command("sed -i 's/a/b/' file").is_err());
        assert!(ensure_readonly_command("cargo test").is_err());
        assert!(ensure_readonly_command("Remove-Item file").is_err());
        assert!(ensure_readonly_command("Set-Content file value").is_err());
        assert!(ensure_readonly_command("winget install foo").is_err());
    }

    #[test]
    fn foreground_wait_accepts_zero() {
        assert_eq!(foreground_wait_seconds(&json!({})), 30);
        assert_eq!(foreground_wait_seconds(&json!({"timeout_seconds": 0})), 0);
        assert_eq!(
            foreground_wait_seconds(&json!({"timeout_seconds": 200})),
            120
        );
    }

    #[tokio::test]
    async fn readonly_command_runs_with_platform_shell() {
        #[cfg(windows)]
        let command = "Write-Output hello";
        #[cfg(not(windows))]
        let command = "printf hello";

        let result = run_readonly_command(json!({"command": command}), String::new())
            .await
            .unwrap();
        let data: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(data["mode"], "foreground");
        assert_eq!(data["success"], true);
        assert_eq!(data["stdout"], "hello");
    }

    #[tokio::test]
    async fn writable_command_promotes_to_background_on_timeout() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let mut config = AppConfig::default();
        config.tools.background_commands_enabled = true;
        config.tools.background_command_timeout_seconds = 0;
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        #[cfg(windows)]
        let command = "Start-Sleep -Seconds 5";
        #[cfg(not(windows))]
        let command = "printf 'before\\n'; sleep 5";

        let result = run_command(
            json!({"command": command, "timeout_seconds": 1, "label": "promote-test"}),
            true,
            String::new(),
            ToolProgress::new(sender),
            &config,
            &paths,
            None,
        )
        .await
        .unwrap();
        let data: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(data["mode"], "background");
        assert_eq!(data["promoted"], true);
        let task_id = data["task_id"].as_str().unwrap().to_string();
        let pid = data["task"]["pid"].as_u64().unwrap() as u32;
        assert!(process_exists(pid));

        // 清理：停止后台进程
        terminate_process(pid, data["task"]["pgid"].as_i64().map(|v| v as i32), true).await;
        let _ = receiver.try_recv();
        assert!(!task_id.is_empty());
    }

    #[tokio::test]
    async fn writable_command_returns_foreground_when_finished() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let mut config = AppConfig::default();
        config.tools.background_commands_enabled = true;
        let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();
        #[cfg(windows)]
        let command = "Write-Output done";
        #[cfg(not(windows))]
        let command = "printf 'done\\n'";

        let result = run_command(
            json!({"command": command, "timeout_seconds": 10}),
            true,
            String::new(),
            ToolProgress::new(sender),
            &config,
            &paths,
            None,
        )
        .await
        .unwrap();
        let data: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(data["mode"], "foreground");
        assert!(data["stdout"].as_str().unwrap().contains("done"));
    }
}
