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

/** GET /api/agent-options 响应体。 */
export type AgentOptions = {
  tools: AgentToolOption[];
  skills: AgentSkillOption[];
};
