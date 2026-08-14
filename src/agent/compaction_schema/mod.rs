mod assemble;
mod catalog;
mod instruction;
mod section;
mod transcript_pointer;
mod verbatim_user;

use std::sync::LazyLock;

pub(crate) use assemble::assemble;
pub(crate) use transcript_pointer::transcript_pointer;
pub(crate) use verbatim_user::{user_messages_section, DEFAULT_USER_SECTION_BUDGET};

/// 回放路径追加在会话末尾的压缩指令。
///
/// 组装一次后复用：它参与上下文余量计算，每次调用都重拼既浪费也会让
/// 长度在同一轮内出现细微差异。
pub(crate) static REPLAY_INSTRUCTION: LazyLock<String> =
    LazyLock::new(instruction::replay_instruction);

/// 返回独立请求路径的默认系统提示词与输入模板。
///
/// 两条路径共用同一份小节目录，因此走哪条路径都产出同一种形态。
///
/// 参数:
/// - 无
///
/// 返回:
/// - （系统提示词，输入模板）
pub(crate) fn standalone_template() -> (String, String) {
    instruction::standalone_template()
}
