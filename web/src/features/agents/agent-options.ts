import type { AppConfig } from "../../api/contracts";
import type { AgentOptions } from "../settings/agents/agents-types";
import type { AgentChoice, AgentProfile } from "./agent-types";
import { text, type Locale } from "../i18n/locale";

export const DEFAULT_AGENT_ID = "default";

/**
 * 从应用配置中读取 Agent 档案。
 *
 * @param config 应用配置
 * @returns 已配置的 Agent 档案
 */
export function readAgentProfiles(config: AppConfig): AgentProfile[] {
  const value = (config as { agents?: AgentProfile[] }).agents;
  return Array.isArray(value) ? value : [];
}

/**
 * 构造继承当前运行行为的默认 Agent 档案。
 *
 * @param options 当前可用工具和 Skills
 * @param locale 当前界面语言
 * @returns 完整的默认 Agent 档案
 */
export function buildDefaultAgent(options: AgentOptions, locale: Locale = "zh-CN"): AgentProfile {
  return {
    id: DEFAULT_AGENT_ID,
    name: text(locale, "Default Agent", "默认 Agent"),
    description: text(locale, "Inherit the current global configuration", "继承当前全局配置"),
    system_prompt: "",
    enabled_tools: options.tools.map((tool) => tool.name),
    skills_full: options.skills.map((skill) => skill.name),
    skills_named: [],
    provider_id: "",
    model: "",
    thinking_level: "auto",
    register_to_main: false,
    load_instruction_files: true
  };
}

/**
 * 构造主界面可选择的 Agent，配置缺失时补充虚拟默认项。
 *
 * @param config 应用配置
 * @param locale 当前界面语言
 * @returns 主界面 Agent 选项
 */
export function buildAgentChoices(config: AppConfig, locale: Locale = "zh-CN"): AgentChoice[] {
  const profiles = readAgentProfiles(config);
  const choices = [
    { id: DEFAULT_AGENT_ID, name: text(locale, "Default Agent", "默认 Agent") },
    { id: "general", name: text(locale, "Coding Agent", "代码 Agent") },
    { id: "explore", name: text(locale, "Explore Agent", "探索 Agent") },
    { id: "gateway", name: text(locale, "Gateway Agent", "网关 Agent") }
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

/**
 * 从本地偏好和有效选项中解析当前 Agent。
 *
 * @param choices 有效 Agent 选项
 * @param preferredId 本地偏好标识
 * @returns 当前 Agent；没有选项时返回空
 */
export function resolveAgentChoice(choices: AgentChoice[], preferredId: string | null): AgentChoice | null {
  return choices.find((choice) => choice.id === preferredId)
    ?? choices.find((choice) => choice.id === "general")
    ?? choices.find((choice) => choice.id === DEFAULT_AGENT_ID)
    ?? choices[0]
    ?? null;
}
