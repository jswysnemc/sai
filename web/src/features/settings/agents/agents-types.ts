export type { AgentProfile } from "../../agents/agent-types";

/** 内置工具选项，含用途分组与摘要。 */
export type AgentToolOption = {
  name: string;
  group: string;
  group_label?: string;
  description?: string;
};

/** Skill 选项，含名称与描述。 */
export type AgentSkillOption = {
  name: string;
  description: string;
};

/** 一个可开关的内置提示词分段。 */
export type PromptSectionOption = {
  id: string;
  label_en: string;
  label_zh: string;
  hint_en: string;
  hint_zh: string;
};

/** GET /api/agent-options 响应体。 */
export type AgentOptions = {
  tools: AgentToolOption[];
  skills: AgentSkillOption[];
  /** 内置提示词分段清单，由后端给出 */
  prompt_sections?: PromptSectionOption[];
};
