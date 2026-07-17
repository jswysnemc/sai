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
