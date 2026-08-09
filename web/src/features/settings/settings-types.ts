import type { AppConfig, ProviderConfig } from "../../api/contracts";
import type { LucideIcon } from "lucide-react";

/** 设置页 section 标识。 */
export type SettingsSectionId =
  | "providers"
  | "agents"
  | "cli-tools"
  | "web-search"
  | "runtime"
  | "prompts"
  | "skills"
  | "git"
  | "appearance"
  | "gateways"
  | "memory"
  | "hooks"
  | "mcp"
  | "session-data"
  | "usage"
  | "advanced";

/** 侧栏分组标识。 */
export type SettingsGroupId = "general" | "integrations" | "workspace" | "operations" | "advanced";

/**
 * 分区对全局 AppConfig 的参与方式，顶栏保存、加载骨架与错误条均由此派生。
 *
 * - required: 必须等 AppConfig 加载完成，顶栏常驻保存
 * - optional: 主体功能不依赖 AppConfig，但可能改写其中字段；有待保存修改时露出顶栏保存
 * - none: 完全不读写 AppConfig，顶栏只显示保存提示文案
 */
export type SettingsAppConfigUse = "required" | "optional" | "none";

export type GatewayId = "qq" | "weixin";

export type SettingsConfigController = {
  config: AppConfig | null;
  /** 服务端用于表示敏感字段未修改的占位符 */
  secretSentinel: string;
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
  /** 重新拉取配置 */
  retry: () => void;
};

/** 设置分区二级子页注册项。 */
export type SettingsSubviewMeta = {
  /** 子页路由段，如 permissions */
  id: string;
  labelEn: string;
  labelZh: string;
};

/** 设置 section 注册项。 */
export type SettingsSectionMeta = {
  id: SettingsSectionId;
  group: SettingsGroupId;
  /** 对全局 AppConfig 的参与方式 */
  appConfig: SettingsAppConfigUse;
  labelEn: string;
  labelZh: string;
  descriptionEn: string;
  descriptionZh: string;
  icon: LucideIcon;
  /** 导航搜索关键字（中英混合字面量）。 */
  searchKeys: string[];
  /** 顶栏保存提示（无待保存修改时展示，如「即时生效」「在本节内保存」） */
  saveHintEn?: string;
  saveHintZh?: string;
  /** 二级子页；声明后子页进入 /settings/:sectionId/:subview 路由 */
  subviews?: SettingsSubviewMeta[];
};

/** 侧栏分组元数据。 */
export type SettingsGroupMeta = {
  id: SettingsGroupId;
  labelEn: string;
  labelZh: string;
};
