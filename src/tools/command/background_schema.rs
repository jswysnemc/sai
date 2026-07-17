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
/// - 支持启动、查看、读取、停止和清理的工具说明
pub(super) fn writable_description() -> &'static str {
    t(
        "Manage long-running shell commands as background tasks. Use action=start to launch a command, list to inspect tasks, output to read logs, stop to terminate a task, and cleanup to remove finished records. Set timeout_seconds=0 for a true background task with no automatic timeout.",
        "以后台任务方式管理长时间运行的 shell 命令。使用 action=start 启动命令，list 查看任务，output 读取日志，stop 停止任务，cleanup 清理结束记录。设置 timeout_seconds=0 可创建不会自动超时的真后台任务。",
    )
}

/// 返回后台命令只读工具说明。
///
/// 返回:
/// - 只支持查看和读取日志的工具说明
pub(super) fn readonly_description() -> &'static str {
    t(
        "Inspect managed background commands. Read-only mode supports action=list and action=output only.",
        "检查受管理后台命令。只读模式仅支持 action=list 和 action=output。",
    )
}

/// 返回后台命令完整 schema。
///
/// 返回:
/// - 写入模式使用的 JSON schema
pub(super) fn writable_schema() -> Value {
    schema(&["start", "list", "output", "stop", "cleanup"])
}

/// 返回后台命令只读 schema。
///
/// 返回:
/// - 只读模式使用的 JSON schema
pub(super) fn readonly_schema() -> Value {
    schema(&["list", "output"])
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
                "description": t("Optional task timeout in seconds for action=start. Use 0 to disable automatic timeout.", "action=start 的可选任务超时时间，单位秒。使用 0 表示不自动超时。"),
            },
            "task_id": {
                "type": "string",
                "description": t("Background task id for action=output or action=stop.", "action=output 或 action=stop 的后台任务 ID。"),
            },
            "stream": {
                "type": "string",
                "enum": ["stdout", "stderr", "all"],
                "description": t("Log stream for action=output. Defaults to all.", "action=output 的日志流，默认 all。"),
            },
            "tail_lines": {
                "type": "integer",
                "description": t("Number of last lines for action=output. Defaults to 200, max 2000.", "action=output 返回末尾行数，默认 200，最大 2000。"),
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
