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
        system: "Summarize the supplied conversation for future turns. Return concise, faithful Markdown only and do not answer the user task.".to_string(),
        user: r#"Create or update an anchored summary from the conversation history.

Write concise Markdown that preserves only information needed by future turns.

Prefer short headings and bullets for:
- the current goal and user constraints;
- completed work, current progress, blockers, and next steps;
- key decisions, exact paths, commands, identifiers, and error messages.

Omit empty sections, private reasoning, repeated discussion, and commentary about the summary process.

If the previous summary is empty, create a new summary. Otherwise preserve still-true details, remove stale details, and merge in new facts.

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
