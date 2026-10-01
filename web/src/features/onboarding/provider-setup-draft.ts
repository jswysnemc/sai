import type { AppConfig, ProviderConfig } from "../../api/contracts";
import type { ProviderSetupInput } from "../../api/onboarding";

/** 首次配置草稿，密钥仅保留用户新输入或环境变量引用。 */
export type ProviderSetupDraft = Omit<ProviderSetupInput, "api_key"> & { api_key: string };

/**
 * 【首次配置】【状态判断】只有服务端明确标记未完成时才进入引导。
 * @param config 当前应用配置
 * @returns 是否需要引导；旧版本缺少字段时不重复引导
 */
export function needsProviderSetup(config: AppConfig): boolean {
  return config.provider_setup_complete === false;
}

/**
 * 【首次配置】【模板草稿】从供应商生成可编辑草稿，脱敏占位符不能当作真实密钥提交。
 * @param provider 已有供应商或模板；为空时新建自定义供应商
 * @param secretSentinel 服务端脱敏标记
 * @returns 可编辑表单
 */
export function createProviderSetupDraft(provider: ProviderConfig | undefined, secretSentinel: string): ProviderSetupDraft {
  return {
    provider_id: provider?.id ?? null,
    display_name: provider?.display_name ?? "",
    base_url: provider?.base_url ?? "",
    protocol: provider?.protocol ?? "auto",
    api_key: provider?.api_key && provider.api_key !== secretSentinel ? provider.api_key : "",
    model: provider?.default_model ?? ""
  };
}

/**
 * 【首次配置】【表单校验】在提交前指出缺少的基本字段，凭据解析交给服务端。
 * @param draft 当前草稿
 * @returns 无效字段；基本字段完整时返回 null
 */
export function providerSetupIssue(draft: ProviderSetupDraft): "name" | "url" | "model" | null {
  if (!draft.display_name.trim()) return "name";
  try {
    const url = new URL(draft.base_url.trim());
    if (!["http:", "https:"].includes(url.protocol) || !url.hostname) return "url";
  } catch { return "url"; }
  return draft.model.trim() ? null : "model";
}

/**
 * 【首次配置】【提交转换】整理输入，空白密钥由服务端继续使用已有凭据。
 * @param draft 当前草稿
 * @returns API 输入
 */
export function providerSetupInput(draft: ProviderSetupDraft): ProviderSetupInput {
  return {
    ...draft,
    display_name: draft.display_name.trim(),
    base_url: draft.base_url.trim(),
    model: draft.model.trim(),
    api_key: draft.api_key.trim() || undefined
  };
}
