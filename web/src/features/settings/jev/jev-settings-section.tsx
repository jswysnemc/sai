import type { AppConfig } from "../../../api/contracts";
import { ModelEndpointSettings } from "../model-endpoints/model-endpoint-settings";
import { JevFeatureSettings } from "./jev-feature-settings";
import { SettingsPanel } from "../kit";
import { useI18n } from "../../i18n/use-i18n";
import "./jev-settings.css";

type JevSettingsSectionProps = {
  config: AppConfig;
  secretSentinel: string;
  dirty: boolean;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 【Jev设置】【分区入口】在同页组合接入状态、决策参数与接入编辑。
 * @param props 应用配置、密钥占位符、草稿状态与更新回调
 * @returns Jev 设置工作台
 */
export function JevSettingsSection({ config, secretSentinel, dirty, onConfigChange }: JevSettingsSectionProps) {
  const { t } = useI18n();
  return <div className="grid gap-4">
    <JevFeatureSettings config={config} dirty={dirty} onConfigChange={onConfigChange} />
    <SettingsPanel title={t("Manage connections", "管理接入")} description={t("Edit addresses, credentials and models on this page.", "在本页维护请求地址、凭据与模型。")}>
      <ModelEndpointSettings kind="jev" config={config} secretSentinel={secretSentinel} onChange={onConfigChange} />
    </SettingsPanel>
  </div>;
}
