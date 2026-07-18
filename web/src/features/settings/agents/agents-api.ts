import { apiRequest } from "../../../api/client";
import type { AgentOptions } from "./agents-types";

/**
 * 拉取 Agent 配置可选的内置工具与 skills 列表。
 *
 * @returns 工具选项（含分组）与 skill 选项（名称与描述）
 */
export function fetchAgentOptions(): Promise<AgentOptions> {
  return apiRequest<AgentOptions>("/api/agent-options");
}

/**
 * 后台发现 MCP 动态工具，不参与 Agent 设置首屏加载。
 *
 * @returns MCP 动态工具选项
 */
export function fetchAgentMcpOptions(): Promise<AgentOptions> {
  return apiRequest<AgentOptions>("/api/agent-options/mcp");
}

/**
 * 合并本地与 MCP Agent 选项，并按名称去重。
 *
 * @param local 已经展示的本地工具与 Skills
 * @param remote 后台发现的 MCP 工具
 * @returns 合并后的选项
 */
export function mergeAgentOptions(local: AgentOptions, remote: AgentOptions): AgentOptions {
  const tools = [...local.tools];
  const knownTools = new Set(tools.map((tool) => tool.name));
  for (const tool of remote.tools) {
    if (!knownTools.has(tool.name)) {
      knownTools.add(tool.name);
      tools.push(tool);
    }
  }
  const skills = [...local.skills];
  const knownSkills = new Set(skills.map((skill) => skill.name));
  for (const skill of remote.skills) {
    if (!knownSkills.has(skill.name)) {
      knownSkills.add(skill.name);
      skills.push(skill);
    }
  }
  return { tools, skills };
}
