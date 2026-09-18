use crate::cli::repl_input::ReplInputSubmission;
use crate::cli::repl_runtime::ReplRuntime;
use crate::i18n::text as t;
use crate::ipc::LinkRole;
use crate::tools::subagent_state;
use anyhow::{bail, Result};

/// 【终端】【接管输入】优先将普通提交投递给当前查看的子代理。
/// 参数: runtime 为当前视图，owner_key 为父会话作用域，submission 为已展开附件的提交，role 为会话角色
/// 返回: 主视图返回空值；子代理视图返回投递结果，失败时调用方必须保留草稿
pub(in crate::cli) fn deliver_viewed_submission(
    runtime: &mut ReplRuntime,
    owner_key: &str,
    submission: &ReplInputSubmission,
    role: LinkRole,
) -> Option<Result<()>> {
    let id = runtime.viewing_subagent_id()?;
    let result = deliver(owner_key, &id, submission, role);
    let error = result.as_ref().err().map(ToString::to_string);
    // 1. 【终端】【投递边界】投递成功后不因重绘失败恢复草稿，避免重复发送
    let _ = runtime.show_subagent_input_result(&id, error);
    Some(result)
}

/// 【终端】【消息投递】校验会话角色与附件后写入指定子代理收件箱。
/// 参数: owner_key 为父会话作用域，id 为子代理 ID，submission 为用户提交，role 为会话角色
/// 返回: 投递结果；校验失败不修改任何会话或收件箱
fn deliver(
    owner_key: &str,
    id: &str,
    submission: &ReplInputSubmission,
    role: LinkRole,
) -> Result<()> {
    if role == LinkRole::Observer {
        bail!(t(
            "Send this message from the terminal that holds the subagent session.",
            "请在持有该子代理会话的终端发送消息，当前跟随终端不支持子代理投递。"
        ));
    }
    if submission.chat_input.image_url.is_some() {
        bail!(t(
            "Subagent messages currently support text only; remove the image and retry.",
            "子代理留言目前仅支持文本，请移除图片后重试。"
        ));
    }
    subagent_state::queue_subagent_message_for_owner(
        owner_key,
        id,
        "user",
        &submission.chat_input.message,
    )?;
    Ok(())
}
