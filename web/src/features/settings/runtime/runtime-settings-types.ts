import type { AppConfig } from "../../../api/contracts";

/** 运行时各领域面板共用的草稿配置与更新回调。 */
export type RuntimeSettingsProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};
