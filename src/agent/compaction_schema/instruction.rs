use super::catalog::SUMMARY_SECTIONS;

/// 回放路径的开场白。
///
/// 回放把整段会话原样送出，摘要指令追加在末尾，所以这里指的是"上文"。
const REPLAY_PREAMBLE: &str = r#"--- This message is a direct task, not part of the above conversation ---

Your context is about to be cleared. Write a structured summary of the conversation above, so that the next turn can continue this work without having seen any of it."#;

/// 独立请求路径的开场白。
///
/// 独立请求把历史渲染成文本塞进一条用户消息，所以这里指的是"下文"。
const STANDALONE_PREAMBLE: &str = r#"Your context is about to be cleared. Write a structured summary of the conversation history given below, so that the next turn can continue this work without having seen any of it."#;

/// 独立请求路径的系统提示词。
const STANDALONE_SYSTEM: &str = "You are the same assistant that produced the conversation being summarized. Produce a structured summary using the exact section headings you are given. Return plain text only: do not call tools, and do not continue the user's task.";

/// 九节格式的通用约定。
///
/// 标题必须逐字复现是硬要求而非风格偏好：程序按标题文本定位插入点，
/// 标题一旦被改写或翻译，机器填充的那一节就会落到错误位置。
const FORMAT_CONTRACT: &str = r#"Use exactly the section headings listed below, in this order, each on its own line as a level-2 markdown heading. Reproduce every heading in English verbatim, including its number — they are structural markers that later steps locate by exact text. Write the content itself in the language the conversation has been using; do not switch to English just because these instructions happen to be in English.

Every section is required. If a section genuinely has nothing to record, keep its heading and write one line saying so. Do not omit a heading, and do not pad a thin section to make it look fuller — length should follow the task, only the structure is fixed."#;

/// 结尾的诚实性与输出约定。
const CLOSING_CONTRACT: &str = r#"Be honest about uncertainty. If an earlier step claimed something was done but was never verified (tests "passing", a fix "working", a file "created"), record it as unverified rather than as fact.

Return the summary as plain text only: do not call any tool, and do not continue the user's task here."#;

/// 组装九节的标题与填写要求。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 每节一段的说明文本
fn sections_block() -> String {
    SUMMARY_SECTIONS
        .iter()
        .map(|section| format!("{}\n{}", section.heading(), section.guidance))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// 返回回放路径追加在会话末尾的压缩指令。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 完整指令文本
pub(crate) fn replay_instruction() -> String {
    format!(
        "{REPLAY_PREAMBLE}\n\n{FORMAT_CONTRACT}\n\n{}\n\n{CLOSING_CONTRACT}",
        sections_block()
    )
}

/// 返回独立请求路径的系统提示词与输入模板。
///
/// 模板保留 `previous_summary` 与 `history` 两个变量，与配置校验约定一致。
///
/// 参数:
/// - 无
///
/// 返回:
/// - （系统提示词，输入模板）
pub(crate) fn standalone_template() -> (String, String) {
    let user = format!(
        "{STANDALONE_PREAMBLE}\n\n{FORMAT_CONTRACT}\n\n{}\n\n{CLOSING_CONTRACT}\n\nIf a previous summary appears below, fold its still-relevant content into the new one: it is being cleared too, so anything you leave out is lost.\n\n<previous-summary>\n{{{{previous_summary}}}}\n</previous-summary>\n\n<conversation-history>\n{{{{history}}}}\n</conversation-history>",
        sections_block()
    );
    (STANDALONE_SYSTEM.to_string(), user)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::compaction_schema::catalog::machine_filled_section;

    /// 验证回放指令包含全部九节标题。
    #[test]
    fn replay_instruction_lists_every_heading() {
        let instruction = replay_instruction();

        for section in SUMMARY_SECTIONS {
            assert!(
                instruction.contains(&section.heading()),
                "缺少小节：{}",
                section.title
            );
        }
    }

    /// 验证回放指令禁止调用工具。
    ///
    /// 回放把会话自己的工具定义一并送出，少了这条约束模型会继续干活。
    #[test]
    fn replay_instruction_forbids_tool_calls() {
        assert!(replay_instruction().contains("do not call any tool"));
    }

    /// 验证独立模板保留了两个必需变量。
    ///
    /// 变量名与 COMPACTION_VARIABLES 的校验一一对应，漏一个配置就加载失败。
    #[test]
    fn standalone_template_keeps_required_variables() {
        let (_, user) = standalone_template();

        assert!(user.contains("{{previous_summary}}"));
        assert!(user.contains("{{history}}"));
    }

    /// 验证独立模板同样带上九节标题。
    ///
    /// 两条路径必须产出同一种形态，否则走哪条路径决定了摘要长什么样。
    #[test]
    fn standalone_template_lists_every_heading() {
        let (_, user) = standalone_template();

        for section in SUMMARY_SECTIONS {
            assert!(user.contains(&section.heading()), "缺少小节：{}", section.title);
        }
    }

    /// 验证指令告知模型不要自行撰写机器填充节。
    #[test]
    fn instruction_tells_the_model_to_skip_the_machine_filled_section() {
        let instruction = replay_instruction();
        let anchor = machine_filled_section();

        assert!(instruction.contains(anchor.guidance));
        assert!(anchor.guidance.contains("Do not write this section yourself"));
    }

    /// 验证系统提示词不含变量占位。
    ///
    /// 系统位不参与变量渲染，混进占位符会原样发给模型。
    #[test]
    fn standalone_system_prompt_has_no_placeholders() {
        let (system, _) = standalone_template();

        assert!(!system.contains("{{"));
    }

    /// 验证指令长度不失控。
    ///
    /// 指令是每次压缩都要重发的固定成本，且回放路径要按它的长度算余量。
    /// 往小节里不断加要求很容易让它悄悄膨胀，这里设一道上界。
    #[test]
    fn instruction_stays_within_a_sane_size() {
        let size = replay_instruction().chars().count();

        assert!(size < 6_000, "指令长度 {size} 超出预期");
    }
}
