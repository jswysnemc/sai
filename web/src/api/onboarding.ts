import { apiRequest } from "./api-request";
import type { ConfigResponse } from "./contracts";

/** 首次供应商配置输入；provider_id 为空表示新建，省略密钥表示保留已有凭据。 */
export type ProviderSetupInput = {
  provider_id: string | null;
  display_name: string;
  base_url: string;
  protocol: string;
  api_key?: string;
  model: string;
};

/**
 * 【首次配置】【保存请求】保存供应商与两端共享的完成状态。
 * @param input 用户确认的连接配置
 * @returns 脱敏配置响应
 */
export function completeProviderSetup(input: ProviderSetupInput): Promise<ConfigResponse> {
  return apiRequest<ConfigResponse>("/api/onboarding/provider", { method: "POST", body: JSON.stringify(input) });
}
