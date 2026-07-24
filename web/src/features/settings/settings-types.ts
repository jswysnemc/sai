import type { AppConfig, ProviderConfig } from "../../api/contracts";
import type { LucideIcon } from "lucide-react";

/** 设置页 section 标识。 */
export type SettingsSectionId =
  | "providers"
  | "agents"
  | "plugins"
  | "runtime"
  | "skills"
  | "git"
  | "appearance"
  | "gateways"
  | "memory"
  | "hooks"
  | "mcp"
  | "usage"
  | "advanced";

/** 侧栏分组标识。 */
export type SettingsGroupId = "general" | "integrations" | "workspace" | "operations" | "advanced";

/**
 * 设置面类型：决定顶栏保存语义与脏状态来源。
 *
 * - app-config: 全局 AppConfig，使用顶栏 Save
 * - local-config: 独立配置文档，section 内保存
 * - client-pref: 浏览器偏好，即时生效
 * - operations: 运维操作面
 * - analytics: 只读统计
 */
export type SettingsSurfaceKind =
  | "app-config"
  | "local-config"
  | "client-pref"
  | "operations"
  | "analytics";

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

/** 设置 section 注册项。 */
export type SettingsSectionMeta = {
  id: SettingsSectionId;
  group: SettingsGroupId;
  kind: SettingsSurfaceKind;
  labelEn: string;
  labelZh: string;
  descriptionEn: string;
  descriptionZh: string;
  icon: LucideIcon;
  /** 导航搜索关键字（中英混合字面量）。 */
  searchKeys: string[];
};

/** 侧栏分组元数据。 */
export type SettingsGroupMeta = {
  id: SettingsGroupId;
  labelEn: string;
  labelZh: string;
};
