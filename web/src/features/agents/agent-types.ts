export type AgentProfile = {
  id: string;
  name: string;
  description: string;
  system_prompt: string;
  enabled_tools: string[];
  skills_full: string[];
  skills_named: string[];
  provider_id: string;
  model: string;
  thinking_level: string;
  register_to_main: boolean;
  load_instruction_files: boolean;
};

export type AgentChoice = {
  id: string;
  name: string;
};
