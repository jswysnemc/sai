import type { AppConfig, ProviderConfig } from "../../api/contracts";

export type SettingsSectionId = "providers" | "agents" | "plugins" | "runtime" | "git" | "appearance" | "gateways" | "memory" | "hooks" | "mcp" | "advanced";
export type GatewayId = "qq" | "weixin";

export type SettingsConfigController = {
  config: AppConfig | null;
  raw: string;
  dirty: boolean;
  loading: boolean;
  saving: boolean;
  error: Error | null;
  saved: boolean;
  updateConfig: (config: AppConfig) => void;
  updateRaw: (raw: string) => void;
  updateProvider: (index: number, patch: Partial<ProviderConfig>) => void;
  updateGateway: (gateway: GatewayId, patch: Record<string, unknown>) => void;
  saveConfig: () => Promise<void>;
};
