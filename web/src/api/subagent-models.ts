import { apiRequest } from "./client";

export type SubagentModelChoice = {
  provider_id: string;
  model: string;
  thinking_level: string;
};

export type SubagentModelProfile = SubagentModelChoice & { id: string; name: string };
export type SubagentModelSettings = {
  defaults: SubagentModelChoice;
  profiles: SubagentModelProfile[];
};

/** 读取子任务共享设置和类型覆盖，无参数，返回设置快照。 */
export function loadSubagentModels(): Promise<SubagentModelSettings> {
  return apiRequest("/api/subagents/model-settings");
}

/**
 * 保存子任务模型和思考设置。
 * @param profileId 子任务类型，null 表示共享默认值
 * @param selection 模型和思考选择
 * @returns 保存后的完整设置
 */
export function saveSubagentModels(profileId: string | null, selection: SubagentModelChoice): Promise<SubagentModelSettings> {
  return apiRequest("/api/subagents/model-settings", {
    method: "PUT",
    body: JSON.stringify({ profile_id: profileId, ...selection })
  });
}
