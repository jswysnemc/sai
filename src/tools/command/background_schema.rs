use crate::i18n::text as t;
use serde_json::{json, Value};

/// 返回后台命令统一工具名称。
///
/// 返回:
/// - 模型侧暴露的后台命令工具名
pub(crate) fn background_tool_name() -> &'static str {
    "background_command"
}

/// 返回后台命令完整工具说明。
///
/// 返回:
/// - 支持启动、查看、读取、等待、停止和清理的工具说明
pub(super) fn writable_description() -> &'static str {
    t(
        "Manage long-running shell commands as background tasks. Prefer run_command for ordinary work; use action=start for immediate background execution. action=wait waits at most 60 seconds, then returns recent logs if still running. Inspect needs_attention results before waiting again. Session tasks automatically request inspection after 90 seconds without output or 10 minutes of runtime; reminders are at least 5 minutes apart. action=start timeout_seconds=0 disables the task lifetime timeout. Use output/list/stop/cleanup to inspect and manage tasks.",
        "以后台任务方式管理长时间运行的 shell 命令。普通命令优先用 run_command；立即后台运行使用 action=start。action=wait 每次最多等待 60 秒，未结束时返回近期日志；收到 needs_attention 后先检查进展再决定是否继续等待。会话任务连续 90 秒没有输出或运行超过 10 分钟时自动提醒检查，同一任务的提醒至少间隔 5 分钟。action=start 的 timeout_seconds=0 表示任务不自动超时。使用 output/list/stop/cleanup 查看和管理任务。",
    )
}

/// 返回后台命令只读工具说明。
///
/// 返回:
/// - 只支持查看、读取和等待的工具说明
pub(super) fn readonly_description() -> &'static str {
    t(
        "Inspect managed background commands. Read-only mode supports action=list, action=output, and action=wait.",
        "检查受管理后台命令。只读模式支持 action=list、action=output 和 action=wait。",
    )
}

/// 返回后台命令完整 schema。
///
/// 返回:
/// - 写入模式使用的 JSON schema
pub(super) fn writable_schema() -> Value {
    schema(&["start", "list", "output", "wait", "stop", "cleanup"])
}

/// 返回后台命令只读 schema。
///
/// 返回:
/// - 只读模式使用的 JSON schema
pub(super) fn readonly_schema() -> Value {
    schema(&["list", "output", "wait"])
}

/// 构造后台命令工具 schema。
///
/// 参数:
/// - `actions`: 允许的 action 列表
///
/// 返回:
/// - JSON schema
fn schema(actions: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": {
            "action": {
                "type": "string",
                "enum": actions,
                "description": t("Operation to perform.", "要执行的操作。"),
            },
            "command": {
                "type": "string",
                "description": t("Shell command for action=start.", "action=start 时要运行的 shell 命令。"),
            },
            "cwd": {
                "type": "string",
                "description": t("Optional working directory for action=start. Defaults to current workspace.", "action=start 的可选工作目录，默认当前工作区。"),
            },
            "label": {
                "type": "string",
                "description": t("Optional human-readable label for action=start.", "action=start 的可选人类可读标签。"),
            },
            "timeout_seconds": {
                "type": "integer",
                "minimum": 0,
                "description": t("Optional seconds for action=start or action=wait. For start, 0 disables automatic task timeout; wait clamps to 1-60 seconds and returns status and recent logs when the interval ends.", "action=start 或 action=wait 的可选秒数。start 使用 0 表示任务不自动超时；wait 限制在 1-60 秒，等待结束时返回状态及近期日志。"),
            },
            "task_id": {
                "type": "string",
                "description": t("Background task id for action=output, action=wait, action=stop, or action=cleanup. Omit it for action=wait to wait for any running task, or for action=cleanup to clean all stopped tasks.", "action=output、action=wait、action=stop 或 action=cleanup 的后台任务 ID。action=wait 可省略以等待任意运行中的任务；action=cleanup 可省略以清理所有已停止任务。"),
            },
            "stream": {
                "type": "string",
                "enum": ["stdout", "stderr", "all"],
                "description": t("Log stream for action=output. Defaults to all.", "action=output 的日志流，默认 all。"),
            },
            "max_lines": {
                "type": "integer",
                "description": t("Alias of head_lines: max lines to return from the start. Defaults to 50.", "与 head_lines 相同：从开头返回的最大行数，默认 50。"),
            },
            "head_lines": {
                "type": "integer",
                "description": t("Number of leading lines for action=output. Defaults to 50, max 2000.", "action=output 开头行数，默认 50，最大 2000。"),
            },
            "tail_lines": {
                "type": "integer",
                "description": t("Number of lines for action=output. Defaults to first 50 (head). Pass tail_lines alone to read from the end. Max 2000.", "action=output 默认读取前 50 行；仅传 tail_lines 时从末尾读。最大 2000。"),
            },
            "force": {
                "type": "boolean",
                "description": t("Force kill immediately for action=stop.", "action=stop 时立即强制终止。"),
            },
            "remove_logs": {
                "type": "boolean",
                "description": t("Whether action=cleanup removes logs for cleaned tasks.", "action=cleanup 是否删除被清理任务的日志。"),
            }
        },
        "required": ["action"],
        "additionalProperties": false
    })
}
