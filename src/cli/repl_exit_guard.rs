//! 退出 REPL 前清点当前会话仍在运行的子智能体与后台命令，并按用户选择停止。

use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::question::{QuestionOption, QuestionPrompt, QuestionRequest, QuestionResponse};
use crate::tools::command::{process_exists, BackgroundCommandStore, BackgroundCommandTask};
use crate::tools::subagent_state::{cancel_subagent_for_owner, list_subagents_for_owner};
use std::path::Path;

/// 选项值：停止全部后台工作后退出。
const CHOICE_STOP: &str = "stop";
/// 选项值：直接退出，后台命令继续运行。
const CHOICE_LEAVE: &str = "leave";
/// 选项值：取消退出。
const CHOICE_STAY: &str = "stay";
/// 提示中最多列出的工作名称。
const SHOWN_NAMES: usize = 4;

/// 一项仍在运行的后台工作。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct WorkItem {
    pub(super) id: String,
    pub(super) label: String,
}

/// 仍在运行、关闭会话前需要告诉用户的后台工作。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct RunningWork {
    /// 运行中或空闲待命的子智能体；随 REPL 进程一起结束
    pub(super) subagents: Vec<WorkItem>,
    /// 进程仍存活的后台命令；独立进程组，REPL 退出后会继续运行
    pub(super) commands: Vec<WorkItem>,
}

impl RunningWork {
    /// 【退出确认】【空判断】判断是否没有任何后台工作。
    /// @returns 没有工作时为 true
    pub(super) fn is_empty(&self) -> bool {
        self.subagents.is_empty() && self.commands.is_empty()
    }
}

/// 用户在退出确认中的选择。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExitChoice {
    /// 停止全部后台工作后退出
    StopAndExit,
    /// 直接退出，后台命令继续运行
    LeaveRunning,
    /// 留在会话里
    Stay,
}

/// 【退出确认】【工作清点】收集当前会话仍在运行的子智能体与后台命令。
///
/// 空闲待命的持久子智能体同样会随退出结束，一并列出；
/// 记录为运行中但进程已经不在的后台命令不计入。
///
/// @param paths 为 Sai 路径；session_id 为会话标识；state_dir 为子智能体归属目录
/// @returns 仍在运行的工作
pub(super) fn running_work(paths: &SaiPaths, session_id: &str, state_dir: &Path) -> RunningWork {
    let owner_key = state_dir.display().to_string();
    let subagents = list_subagents_for_owner(&owner_key)
        .into_iter()
        .filter(|snapshot| matches!(snapshot.status.as_str(), "running" | "idle"))
        .map(|snapshot| {
            let label = snapshot.description.trim();
            WorkItem {
                label: if label.is_empty() {
                    snapshot.subagent_type.clone()
                } else {
                    label.to_string()
                },
                id: snapshot.id,
            }
        })
        .collect();
    let commands = BackgroundCommandStore::new(paths.state_dir.clone())
        .load()
        .unwrap_or_default()
        .into_iter()
        .filter(|task| task.owned_by_session(session_id) && task.status == "running")
        .filter(|task| task.pid == 0 || process_exists(task.pid))
        .map(|task| WorkItem {
            id: task.id.clone(),
            label: command_label(&task),
        })
        .collect();
    RunningWork {
        subagents,
        commands,
    }
}

/// 【退出确认】【命令标签】取后台命令的短标签。
/// @param task 为后台命令
/// @returns 优先使用标签，否则使用命令前 48 个字符
fn command_label(task: &BackgroundCommandTask) -> String {
    let label = task.label.trim();
    if !label.is_empty() {
        return label.to_string();
    }
    task.command.chars().take(48).collect()
}

/// 【退出确认】【工作摘要】列出前几项工作名称。
/// @param work 为仍在运行的工作
/// @returns 用顿号或逗号连接的名称，超出部分写明数量
fn work_names(work: &RunningWork) -> String {
    let names: Vec<&str> = work
        .subagents
        .iter()
        .chain(&work.commands)
        .map(|item| item.label.as_str())
        .collect();
    let separator = crate::i18n::text(", ", "、");
    let mut text = names
        .iter()
        .take(SHOWN_NAMES)
        .copied()
        .collect::<Vec<_>>()
        .join(separator);
    let extra = names.len().saturating_sub(SHOWN_NAMES);
    if extra > 0 {
        text.push_str(&if crate::i18n::is_zh() {
            format!(" 等 {} 项", names.len())
        } else {
            format!(", plus {extra} more")
        });
    }
    text
}

/// 【退出确认】【问题构造】生成停止、保留、取消三选一的退出确认。
///
/// 子智能体运行在 REPL 进程内，任何退出都会结束它们；只有后台命令可以保留。
/// 没有后台命令时不提供“保留后台命令”选项，避免给出做不到的承诺。
///
/// @param work 为仍在运行的工作
/// @returns 结构化提问
pub(super) fn exit_question(work: &RunningWork) -> QuestionRequest {
    let t = crate::i18n::text;
    let question = if crate::i18n::is_zh() {
        format!(
            "还有 {} 个子智能体、{} 个后台命令在运行：{}。退出前要怎么处理？",
            work.subagents.len(),
            work.commands.len(),
            work_names(work)
        )
    } else {
        format!(
            "{} subagent(s) and {} background command(s) are still running: {}. What should happen before exit?",
            work.subagents.len(),
            work.commands.len(),
            work_names(work)
        )
    };
    let mut options = vec![QuestionOption {
        label: t("Stop all and exit", "全部停止并退出").to_string(),
        description: t(
            "Cancel subagents and stop background commands, then exit.",
            "取消子智能体并停止后台命令，然后退出。",
        )
        .to_string(),
        value: Some(CHOICE_STOP.to_string()),
    }];
    if !work.commands.is_empty() {
        options.push(QuestionOption {
            label: t("Exit, keep commands running", "退出，保留后台命令").to_string(),
            description: if work.subagents.is_empty() {
                t(
                    "Background commands keep running; manage them with sai ps.",
                    "后台命令继续运行，可用 sai ps 管理。",
                )
            } else {
                t(
                    "Background commands keep running; subagents still end with the session.",
                    "后台命令继续运行；子智能体仍会随会话结束。",
                )
            }
            .to_string(),
            value: Some(CHOICE_LEAVE.to_string()),
        });
    }
    options.push(QuestionOption {
        label: t("Cancel", "取消，留在会话").to_string(),
        description: t("Do not exit.", "不退出。").to_string(),
        value: Some(CHOICE_STAY.to_string()),
    });
    QuestionRequest {
        questions: vec![QuestionPrompt {
            header: t("Background work running", "后台工作仍在运行").to_string(),
            question,
            options,
            multiple: false,
            custom: false,
            required: true,
            default_answers: vec![CHOICE_STOP.to_string()],
            validation: None,
        }],
    }
}

/// 【退出确认】【选择解析】把提问回答转换成退出选择；取消或无法提问时留在会话。
/// @param response 为提问结果
/// @returns 退出选择
pub(super) fn exit_choice(response: &QuestionResponse) -> ExitChoice {
    let answers = match response {
        QuestionResponse::Answered(answers) => answers,
        QuestionResponse::AnsweredWithImages { answers, .. } => answers,
        QuestionResponse::Cancelled | QuestionResponse::Unavailable(_) => return ExitChoice::Stay,
    };
    match answers
        .first()
        .and_then(|answer| answer.first())
        .map(String::as_str)
    {
        Some(CHOICE_STOP) => ExitChoice::StopAndExit,
        Some(CHOICE_LEAVE) => ExitChoice::LeaveRunning,
        _ => ExitChoice::Stay,
    }
}

/// 【退出确认】【停止工作】取消子智能体并停止后台命令，返回停止失败的项目。
///
/// @param paths 为 Sai 路径；config 为应用配置；state_dir 为子智能体归属目录；work 为待停止的工作
/// @returns 停止失败的说明，全部成功时为空
pub(super) async fn stop_work(
    paths: &SaiPaths,
    config: &AppConfig,
    state_dir: &Path,
    work: &RunningWork,
) -> Vec<String> {
    let owner_key = state_dir.display().to_string();
    let mut failures = Vec::new();
    // 1. 子智能体只需撤销任务，不涉及外部进程
    for item in &work.subagents {
        if let Err(error) = cancel_subagent_for_owner(&owner_key, &item.id) {
            failures.push(format!("{}: {error:#}", item.label));
        }
    }
    // 2. 后台命令先发终止信号，宽限期后仍未退出再强制结束
    for item in &work.commands {
        if let Err(error) =
            crate::tools::command::stop_background_task_for_user(paths, config, &item.id, false)
                .await
        {
            failures.push(format!("{}: {error:#}", item.label));
        }
    }
    failures
}

/// 【退出确认】【文本提示】终端无法弹出选择时的提示：再执行一次退出才关闭。
/// @param work 为仍在运行的工作
/// @returns 给用户看的一行提示
pub(super) fn fallback_notice(work: &RunningWork) -> String {
    if crate::i18n::is_zh() {
        format!(
            "还有 {} 个子智能体、{} 个后台命令在运行（{}）。再执行一次退出将停止它们并关闭会话。",
            work.subagents.len(),
            work.commands.len(),
            work_names(work)
        )
    } else {
        format!(
            "{} subagent(s) and {} background command(s) are still running ({}). Exit again to stop them and close the session.",
            work.subagents.len(),
            work.commands.len(),
            work_names(work)
        )
    }
}

#[cfg(test)]
#[path = "repl_exit_guard_tests.rs"]
mod tests;
