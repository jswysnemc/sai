import type { PromptTemplatesConfig } from "../../../api/contracts";

export type PromptTemplateId = keyof PromptTemplatesConfig;

export type PromptTemplateDefinition = {
  id: PromptTemplateId;
  labelEn: string;
  labelZh: string;
  descriptionEn: string;
  descriptionZh: string;
  variables: Array<{
    name: string;
    descriptionEn: string;
    descriptionZh: string;
  }>;
};

export const DEFAULT_PROMPT_TEMPLATES: PromptTemplatesConfig = {
  git_commit: {
    system: "You write Git commit messages. Output ONLY the commit message body using Conventional Commits (type(scope): subject). Prefer Chinese subject when the change descriptions are Chinese. Keep subject under 72 characters. Optionally add a short body after a blank line. No markdown fences, no quotes, no commentary.",
    user: "Git status:\n{{status}}\n\nDiff summary:\n{{diff}}\n"
  },
  session_title: {
    system: "You name chat sessions. Reply with ONLY a short title (max 24 Chinese characters or 8 English words). No quotes, no punctuation wrappers, no explanation.",
    user: "User message:\n{{user_message}}\n\nAssistant reply preview:\n{{assistant_preview}}\n"
  },
  compaction: {
    system: "Summarize the supplied conversation for future turns. Return concise, faithful Markdown only and do not answer the user task.",
    user: `Create or update an anchored summary from the conversation history.

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
</conversation-history>`
  }
};

export const PROMPT_TEMPLATE_DEFINITIONS: PromptTemplateDefinition[] = [
  {
    id: "git_commit",
    labelEn: "Git commit message",
    labelZh: "Git 提交说明",
    descriptionEn: "Generate a Conventional Commits message from repository status and diff.",
    descriptionZh: "根据仓库状态和差异生成 Conventional Commits 提交说明。",
    variables: [
      { name: "status", descriptionEn: "Repository status summary", descriptionZh: "仓库状态摘要" },
      { name: "diff", descriptionEn: "Staged or working-tree diff", descriptionZh: "暂存区或工作树差异" }
    ]
  },
  {
    id: "session_title",
    labelEn: "Session title",
    labelZh: "会话标题",
    descriptionEn: "Create the first automatic title from the opening exchange.",
    descriptionZh: "根据首轮问题和回答生成首次自动标题。",
    variables: [
      { name: "user_message", descriptionEn: "Opening user message", descriptionZh: "首条用户消息" },
      { name: "assistant_preview", descriptionEn: "Assistant reply preview", descriptionZh: "助手回答预览" }
    ]
  },
  {
    id: "compaction",
    labelEn: "Context compaction",
    labelZh: "上下文压缩",
    descriptionEn: "Summarize earlier turns while preserving state needed by future work.",
    descriptionZh: "压缩早期轮次并保留后续工作需要的状态。",
    variables: [
      { name: "previous_summary", descriptionEn: "Previous anchored summary", descriptionZh: "上一版锚定摘要" },
      { name: "history", descriptionEn: "Conversation history selected for compaction", descriptionZh: "本次参与压缩的会话历史" }
    ]
  }
];

/**
 * 合并服务端模板和前端默认值，兼容尚未写入新字段的旧配置。
 *
 * @param templates 服务端返回的可选模板集合
 * @returns 字段完整的提示词模板集合
 */
export function resolvePromptTemplates(
  templates: Partial<PromptTemplatesConfig> | undefined
): PromptTemplatesConfig {
  return {
    git_commit: { ...DEFAULT_PROMPT_TEMPLATES.git_commit, ...templates?.git_commit },
    session_title: { ...DEFAULT_PROMPT_TEMPLATES.session_title, ...templates?.session_title },
    compaction: { ...DEFAULT_PROMPT_TEMPLATES.compaction, ...templates?.compaction }
  };
}
