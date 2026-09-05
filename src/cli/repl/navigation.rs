use super::*;

/// 选择会话树轮次并重绘该分支历史。
///
/// 参数: `agent` 为当前会话，`runtime` 为终端状态，`turn_id` 为可选目标轮次或 root
/// 返回: 导航与重绘结果，用户取消时正常返回
pub(super) fn select_turn(
    agent: &Agent,
    runtime: &mut ReplRuntime,
    turn_id: Option<String>,
) -> Result<()> {
    // 1. 未指定轮次时打开树界面交互选择
    let target = match turn_id {
        Some(id) if matches!(id.trim(), "root" | "start") => {
            Some(tree_select::TreeSelection::Start)
        }
        Some(id) => Some(tree_select::TreeSelection::Turn(id)),
        None => {
            let picked = tree_select::select_turn_interactively(agent.state());
            // 内联选择 UI 退出后全量重放，清除其占用行的残留
            runtime.redraw()?;
            match picked {
                Ok(target) => target,
                Err(err) => {
                    runtime.record_meta(err.to_string())?;
                    return Ok(());
                }
            }
        }
    };
    let Some(target) = target else {
        runtime.record_meta(t("tree navigation cancelled", "已取消会话树切换").to_string())?;
        return Ok(());
    };
    // 2. 切换活动叶子，随后清屏重放该分支历史
    let result = match &target {
        tree_select::TreeSelection::Start => agent.state().switch_to_session_start(),
        tree_select::TreeSelection::Turn(id) => agent.state().switch_active_leaf(id),
    };
    match result {
        Ok(()) => {
            runtime.clear()?;
            record_repl_history(runtime, agent.state())?;
            let message = if target == tree_select::TreeSelection::Start {
                t(
                    "At session start; the next message starts a new branch",
                    "已返回会话起点，下一条消息将创建新的分支",
                )
            } else {
                t("switched to the selected turn", "已切换到所选轮次")
            };
            runtime.record_meta(message.to_string())?;
        }
        Err(err) => runtime.record_meta(err.to_string())?,
    }
    Ok(())
}

/// 确认撤销上一轮，并将上一轮输入交还输入框。
///
/// 参数: `state` 为会话存储，`runtime` 为终端状态，`pending_undo` 为确认标志，`prefill` 接收恢复的输入
/// 返回: 撤销与回显结果
pub(super) fn undo_turn(
    state: &StateStore,
    runtime: &mut ReplRuntime,
    pending_undo: &mut bool,
    prefill: &mut Option<String>,
) -> Result<()> {
    if !*pending_undo {
        *pending_undo = true;
        runtime.record_meta(
            t(
                "type /undo again to drop the last turn",
                "再次输入 /undo 确认撤销上一轮",
            )
            .to_string(),
        )?;
        return Ok(());
    }
    *pending_undo = false;
    let outcome = state.undo_last_turn()?;
    runtime.record_meta(format!(
        "{}: {}",
        t("undone messages", "已撤销消息数"),
        outcome.removed
    ))?;
    *prefill = outcome.prompt;
    Ok(())
}
