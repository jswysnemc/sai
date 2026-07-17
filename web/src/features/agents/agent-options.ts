import type { AppConfig } from "../../api/contracts";
import type { AgentOptions } from "../settings/agents/agents-types";
import type { AgentChoice, AgentProfile } from "./agent-types";

export const DEFAULT_AGENT_ID = "default";

/** 从应用配置中读取 Agent 档案。 */
export function readAgentProfiles(config: AppConfig): AgentProfile[] {
  const value = (config as { agents?: AgentProfile[] }).agents;
  return Array.isArray(value) ? value : [];
}

/** 构造继承当前运行行为的默认 Agent 档案。 */
export function buildDefaultAgent(options: AgentOptions): AgentProfile {
  return {
    id: DEFAULT_AGENT_ID,
    name: "默认 Agent",
    description: "继承当前全局配置",
    system_prompt: "",
    enabled_tools: options.tools.map((tool) => tool.name),
    skills_full: options.skills.map((skill) => skill.name),
    skills_named: [],
    provider_id: "",
    model: "",
    thinking_level: "auto",
    register_to_main: false
  };
}

/** 构造主界面可选择的 Agent，配置缺失时补充虚拟默认项。 */
export function buildAgentChoices(config: AppConfig): AgentChoice[] {
  const profiles = readAgentProfiles(config);
  const choices = [
    { id: DEFAULT_AGENT_ID, name: "默认 Agent" },
    { id: "general", name: "代码 Agent" },
    { id: "explore", name: "探索 Agent" },
    { id: "gateway", name: "网关 Agent" }
  ];
  for (const profile of profiles) {
    const choice = { id: profile.id, name: profile.name || profile.id };
    const existing = choices.findIndex((item) => item.id === choice.id);
    if (existing >= 0) choices[existing] = choice;
    else choices.push(choice);
  }
  for (const legacy of config.subagent?.profiles ?? []) {
    if (!choices.some((choice) => choice.id === legacy.id)) {
      choices.push({ id: legacy.id, name: legacy.name || legacy.id });
    }
  }
  return choices;
}

/** 从本地偏好和有效选项中解析当前 Agent。 */
export function resolveAgentChoice(choices: AgentChoice[], preferredId: string | null): AgentChoice | null {
  return choices.find((choice) => choice.id === preferredId)
    ?? choices.find((choice) => choice.id === "general")
    ?? choices.find((choice) => choice.id === DEFAULT_AGENT_ID)
    ?? choices[0]
    ?? null;
}
