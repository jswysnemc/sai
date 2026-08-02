use crate::prompts::template::validate_prompt_pair;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const GIT_COMMIT_VARIABLES: &[&str] = &["status", "diff"];
const SESSION_TITLE_VARIABLES: &[&str] = &["user_message", "assistant_preview"];
const COMPACTION_VARIABLES: &[&str] = &["previous_summary", "history"];

/// 单项内部任务的系统提示词和用户输入模板。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptTemplateConfig {
    #[serde(default)]
    pub system: String,
    #[serde(default)]
    pub user: String,
}

/// Sai 内部模型任务使用的可编辑提示词集合。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptTemplatesConfig {
    #[serde(default = "default_git_commit_template")]
    pub git_commit: PromptTemplateConfig,
    #[serde(default = "default_session_title_template")]
    pub session_title: PromptTemplateConfig,
    #[serde(default = "default_compaction_template")]
    pub compaction: PromptTemplateConfig,
}

impl Default for PromptTemplatesConfig {
    /// 创建包含全部内置提示词的默认配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - Git 提交说明、会话标题和上下文压缩的默认模板集合
    fn default() -> Self {
        Self {
            git_commit: default_git_commit_template(),
            session_title: default_session_title_template(),
            compaction: default_compaction_template(),
        }
    }
}

/// 校验全部内部提示词及变量约束。
///
/// 参数:
/// - `templates`: 待校验提示词集合
///
/// 返回:
/// - 全部模板合法时返回空结果
pub(crate) fn validate_prompt_templates(templates: &PromptTemplatesConfig) -> Result<()> {
    validate_prompt_pair(
        &templates.git_commit,
        GIT_COMMIT_VARIABLES,
        GIT_COMMIT_VARIABLES,
    )
    .context("prompt.templates.git_commit is invalid")?;
    validate_prompt_pair(
        &templates.session_title,
        SESSION_TITLE_VARIABLES,
        SESSION_TITLE_VARIABLES,
    )
    .context("prompt.templates.session_title is invalid")?;
    validate_prompt_pair(
        &templates.compaction,
        COMPACTION_VARIABLES,
        COMPACTION_VARIABLES,
    )
    .context("prompt.templates.compaction is invalid")?;
    Ok(())
}

/// 返回 Git 提交说明的默认模板。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 提交说明系统提示词和输入模板
fn default_git_commit_template() -> PromptTemplateConfig {
    PromptTemplateConfig {
        system: "You write Git commit messages. Output ONLY the commit message body using Conventional Commits (type(scope): subject). Prefer Chinese subject when the change descriptions are Chinese. Keep subject under 72 characters. Optionally add a short body after a blank line. No markdown fences, no quotes, no commentary.".to_string(),
        user: "Git status:\n{{status}}\n\nDiff summary:\n{{diff}}\n".to_string(),
    }
}

/// 返回会话标题的默认模板。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 标题系统提示词和输入模板
fn default_session_title_template() -> PromptTemplateConfig {
    PromptTemplateConfig {
        system: "You name chat sessions. Reply with ONLY a short title (max 24 Chinese characters or 8 English words). No quotes, no punctuation wrappers, no explanation.".to_string(),
        user: "User message:\n{{user_message}}\n\nAssistant reply preview:\n{{assistant_preview}}\n".to_string(),
    }
}

/// 返回上下文压缩的默认模板。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 压缩系统提示词和输入模板
fn default_compaction_template() -> PromptTemplateConfig {
    PromptTemplateConfig {
        system: "You are the same assistant that produced the conversation below. Write a first-person handoff note to yourself. Return the note as plain text only: do not call tools, and do not answer the user's task here.".to_string(),
        user: r#"You are about to run out of context. Write a first-person handoff note to yourself so you can seamlessly continue this task after the earlier conversation is cleared.

--- This message is a direct task, not part of the above conversation ---

Write the note as your own continuing train of thought — first person, present tense, the way you would reason through the next move. Do not write a third-party report about someone else's work, and do not impose rigid section headings; let the shape follow the task. Write the note in the same language the conversation has been using — do not switch to English just because these instructions happen to be in English.

Make the note self-sufficient: the next turn will see only your most recent user messages and this note — every assistant message, tool call, and tool result above will be gone. In your own words, preserve what you genuinely need to continue:

- What the latest request is actually asking for: your reading of its intent and any ambiguity you have already resolved. The kept user messages are size-capped, so a long request is truncated there: if the latest request is large, preserve the parts at risk of being dropped — above all the actual ask. If several requests are in play, say which one governs the next move.
- The instructions and constraints currently in force (user preferences, project rules, environment and tooling limits) — condensed to what still matters. Keep decisions you have already settled (what you chose and why) separate from questions still open, so you neither silently reopen a closed choice nor treat an undecided point as decided.
- What has actually been done, at high fidelity: the exact commands that were run, the exact file paths touched, and whether each succeeded or failed — and the results themselves, not just the commands: the concrete values returned, the key lines or error text, the schema or signature a lookup revealed, since re-running to recover them may be slow or impossible. Keep only the final working version of any code; drop intermediate attempts and already-resolved errors.
- What you still don't know: context the next step depends on that this conversation never established — files referenced but not yet read, schemas or APIs assumed but unseen, questions the user has not answered. Name these gaps so the next turn checks them instead of assuming.
- The forward plan — and this is the moment to invest in it. Right now you hold more context on this task than you ever will again; the next turn resumes with less, so the plan you commit here is the one it will follow. Give the exact next command or tool call, but don't stop at the next step: set out the remaining sequence to finish, the decisions you have already made for those upcoming steps, the obstacles you can foresee and how you mean to handle them, and any work you can commit to now. Include any required format for the final answer.

Be honest about uncertainty. If an earlier step claimed something was done but was never verified (tests "passing", a fix "working", a file "created"), say so plainly and treat it as unverified rather than fact.

Be concise, and keep the note proportional to the task: a long multi-step task warrants detail, but a trivial or nearly finished exchange needs only a sentence or two — do not pad it out. Include the critical data, identifiers, and references needed to continue, and omit anything that does not change the next move.

If a previous handoff note is present below, fold its still-relevant content into your new note: it will be removed from the context, so anything you omit is lost.

<previous-summary>
{{previous_summary}}
</previous-summary>

<conversation-history>
{{history}}
</conversation-history>"#
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证内置模板满足全部变量约束。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn default_templates_are_valid() {
        validate_prompt_templates(&PromptTemplatesConfig::default()).unwrap();
    }

    /// 验证未知变量会阻止无效配置落盘。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn invalid_variable_is_rejected() {
        let mut templates = PromptTemplatesConfig::default();
        templates.git_commit.user.push_str("\n{{branch}}");

        let error = validate_prompt_templates(&templates).unwrap_err();
        assert!(error.to_string().contains("git_commit"));
    }
}
