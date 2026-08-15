import type { PromptSectionToggles } from "../../api/contracts";

export type AgentProfile = {
  id: string;
  name: string;
  description: string;
  system_prompt: string;
  enabled_tools: string[];
  deferred_tools: string[];
  skills_full: string[];
  skills_named: string[];
  provider_id: string;
  model: string;
  thinking_level: string;
  register_to_main: boolean;
  load_instruction_files: boolean;
  /** 工具白名单是否为最终结果；为真时空列表表示一个工具都不给 */
  tools_exclusive: boolean;
  /** 系统提示词各内置分段的开关；未设置表示全部沿用默认 */
  prompt_sections?: PromptSectionToggles;
};

export type AgentChoice = {
  id: string;
  name: string;
};
