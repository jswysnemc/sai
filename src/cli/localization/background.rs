use crate::i18n::text as t;

/// 本地化后台命令子命令和参数。
///
/// 参数:
/// - `command`: Clap 后台命令
///
/// 返回:
/// - 已本地化的后台命令
pub(super) fn localize_background_commands_command(command: clap::Command) -> clap::Command {
    command
        .mut_subcommand("start", |subcommand| {
            subcommand
                .about(t(
                    "Start a managed background command",
                    "启动受管理后台命令",
                ))
                .mut_arg("cwd", |arg| arg.help(t("Working directory", "工作目录")))
                .mut_arg("label", |arg| arg.help(t("Display label", "显示名称")))
                .mut_arg("timeout_seconds", |arg| {
                    arg.help(t("Timeout in seconds", "超时秒数"))
                })
                .mut_arg("no_timeout", |arg| {
                    arg.help(t("Disable timeout", "禁用超时限制"))
                })
                .mut_arg("command", |arg| {
                    arg.help(t("Command and arguments to execute", "要执行的命令和参数"))
                })
        })
        .mut_subcommand("list", |subcommand| {
            subcommand.about(t("List background commands", "列出后台命令"))
        })
        .mut_subcommand("output", |subcommand| {
            subcommand
                .about(t("Read background command output", "读取后台命令输出"))
                .mut_arg("task_id", |arg| {
                    arg.help(t("Background task ID", "后台任务 ID"))
                })
                .mut_arg("stream", |arg| {
                    arg.help(t(
                        "Output stream: all, stdout, or stderr",
                        "输出流：all、stdout 或 stderr",
                    ))
                })
                .mut_arg("tail_lines", |arg| {
                    arg.help(t("Number of trailing lines", "末尾输出行数"))
                })
        })
        .mut_subcommand("stop", |subcommand| {
            subcommand
                .about(t("Stop a background command", "停止后台命令"))
                .mut_arg("task_id", |arg| {
                    arg.help(t("Background task ID", "后台任务 ID"))
                })
                .mut_arg("force", |arg| {
                    arg.help(t("Force immediate termination", "立即强制终止"))
                })
        })
        .mut_subcommand("cleanup", |subcommand| {
            subcommand
                .about(t(
                    "Cleanup finished background commands",
                    "清理已结束后台命令",
                ))
                .mut_arg("remove_logs", |arg| {
                    arg.help(t("Also remove saved logs", "同时删除已保存日志"))
                })
        })
}
