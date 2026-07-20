use super::process::{run_shell_command, run_shell_command_with_progress};
use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::tools::{ToolProgress, ToolRegistry, ToolSpec};
use anyhow::{bail, Result};
use serde_json::{json, Value};

const MAX_COMMAND_OUTPUT_CHARS: usize = 20_000;

/// 注册可写命令执行工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `config`: 应用配置
/// - `allow_command_execution`: 是否允许执行命令
pub(crate) fn register(
    registry: &mut ToolRegistry,
    config: &AppConfig,
    allow_command_execution: bool,
) {
    let shell = config.tools.command_shell.clone();
    registry.register(ToolSpec::new_with_progress(
        "run_command",
        t(
            "Run workspace shell commands for builds, tests, validation, export, inspection, and other command-line programs. Prefer edit_file/write_file for source text edits, but shell redirection, tee, and heredocs are allowed when useful. A program's own -o/--output option may create build artifacts.",
            "运行构建、测试、校验、导出、检查及其他命令行程序。源码文本修改优先使用 edit_file/write_file，但 shell 重定向、tee、heredoc 也允许使用。程序自身通过 -o/--output 生成构建产物属于允许用途。",
        ),
        json!({"type":"object","properties":{"command":{"type":"string","description": t("Complete shell command string. Put pipelines and conditionals in this single field; do not pass argv arrays or separate cwd fields.", "完整 Shell 命令字符串。管道和条件语句都放在此字段中，不要传 argv 数组或额外 cwd 字段。")},"timeout_seconds":{"type":"integer","minimum":1,"maximum":120,"description": t("Optional timeout from 1 to 120 seconds. Defaults to 30.", "可选超时，范围 1 到 120 秒，默认 30 秒。")},"sandbox_permissions":{"type":"string","enum":["use_default","require_escalated"],"description":t("Use require_escalated when the command needs network access or must read or write outside the workspace sandbox.", "命令需要网络访问或必须读写工作区沙箱外部时使用 require_escalated。")},"justification":{"type":"string","description":t("Short reason shown with an elevated permission request.", "提升权限请求中展示的简短原因。")}},"required":["command"],"additionalProperties":false}),
        move |args, progress| {
            let shell = shell.clone();
            async move { run_command(args, allow_command_execution, shell, progress).await }
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
        t("Run an explicitly read-only shell command for inspection. Mutating commands are blocked in plan mode.", "运行明确只读的 shell 命令用于检查。计划模式会阻止修改性命令。"),
        json!({"type":"object","properties":{"command":{"type":"string","description": t("Read-only command to run.", "要运行的只读命令。")},"timeout_seconds":{"type":"integer","description": t("Optional timeout in seconds.", "可选超时时间，单位秒。")}},"required":["command"],"additionalProperties":false}),
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
///
/// 返回:
/// - JSON 格式命令结果
async fn run_command(
    args: Value,
    allowed: bool,
    shell: String,
    progress: ToolProgress,
) -> Result<String> {
    if !allowed {
        bail!("{}", t("command execution is disabled; set skills.allow_command_execution=true in config.jsonc to enable run_command", "命令执行已禁用；请在 config.jsonc 中设置 skills.allow_command_execution=true 以启用 run_command"));
    }
    let command = required(&args, "command")?;
    let timeout = command_timeout(&args);
    let sandboxed = args
        .get("_sai_sandbox")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let output =
        run_shell_command_with_progress(&command, timeout, shell.as_str(), sandboxed, progress)
            .await?;
    command_output(output)
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
    let timeout = command_timeout(&args);
    let output = run_shell_command(&command, timeout, shell.as_str(), false).await?;
    command_output(output)
}

/// 读取并限制命令超时参数。
///
/// 参数:
/// - `args`: 工具参数
///
/// 返回:
/// - 超时秒数
fn command_timeout(args: &Value) -> u64 {
    args.get("timeout_seconds")
        .and_then(Value::as_u64)
        .unwrap_or(30)
        .clamp(1, 120)
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

/// 生成命令输出 JSON。
///
/// 参数:
/// - `output`: 进程输出
///
/// 返回:
/// - JSON 字符串
fn command_output(output: std::process::Output) -> Result<String> {
    let stdout = clip_output(&String::from_utf8_lossy(&output.stdout));
    let stderr = clip_output(&String::from_utf8_lossy(&output.stderr));
    Ok(serde_json::to_string_pretty(
        &json!({"success": output.status.success(), "exit_code": output.status.code(), "stdout": stdout, "stderr": stderr}),
    )?)
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
        assert_eq!(data["success"], true);
        assert_eq!(data["stdout"], "hello");
    }

    #[tokio::test]
    async fn writable_command_reports_output_before_completion() {
        #[cfg(windows)]
        let command = "Write-Output first; Start-Sleep -Milliseconds 50; Write-Output second";
        #[cfg(not(windows))]
        let command = "printf 'first\\n'; sleep 0.05; printf 'second\\n'";
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

        let result = run_command(
            json!({"command": command}),
            true,
            String::new(),
            ToolProgress::new(sender),
        )
        .await
        .unwrap();
        let chunks = std::iter::from_fn(|| receiver.try_recv().ok())
            .filter_map(|message| super::super::progress::decode_command_output(&message))
            .collect::<Vec<_>>();
        let stdout = chunks
            .into_iter()
            .filter(|chunk| chunk.stream == super::super::progress::CommandOutputStream::Stdout)
            .flat_map(|chunk| chunk.bytes)
            .collect::<Vec<_>>();

        let stdout_text = String::from_utf8_lossy(&stdout).replace("\r\n", "\n");
        assert_eq!(stdout_text, "first\nsecond\n");
        assert!(result.contains("first"));
    }
}
