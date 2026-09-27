import type { AppConfig } from "../../../api/contracts";
import { ModelEndpointSettings } from "../model-endpoints/model-endpoint-settings";
import { JevFeatureSettings } from "./jev-feature-settings";
import "./jev-settings.css";

type JevSettingsSectionProps = {
  config: AppConfig;
  /** 当前子页：features / connections */
  subview?: string;
  secretSentinel: string;
  dirty: boolean;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 【Jev设置】【分区入口】按子页渲染功能开关或接入列表。
 * @param props 应用配置、当前子页、密钥占位符、草稿状态与更新回调
 * @returns 当前子页内容
 */
export function JevSettingsSection({ config, subview, secretSentinel, dirty, onConfigChange }: JevSettingsSectionProps) {
  if (subview === "connections") {
    return <ModelEndpointSettings kind="jev" config={config} secretSentinel={secretSentinel} onChange={onConfigChange} />;
  }
  return <JevFeatureSettings config={config} dirty={dirty} onConfigChange={onConfigChange} />;
}
