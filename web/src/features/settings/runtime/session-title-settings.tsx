import type { AppConfig } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import { ModelChoiceSelect } from "../controls/model-choice-select";
import { FieldGrid, SettingsField, SettingsPanel, SwitchField } from "../kit";
import type { RuntimeSettingsProps } from "./runtime-settings-types";

/**
 * 【Web 设置】【会话标题】配置首次回复后的自动命名与标题模型。
 * @param props 应用配置与更新回调
 * @returns 会话标题设置面板
 */
export function SessionTitleSettings({ config, onConfigChange }: RuntimeSettingsProps) {
  const { t } = useI18n();
  const session = config.session ?? {};
  const enabled = session.auto_title_enabled ?? true;

  /**
   * 合并标题设置，保留新会话模型与思考等级。
   * @param patch 修改的会话字段
   * @returns 无返回值
   */
  const update = (patch: Partial<NonNullable<AppConfig["session"]>>) => onConfigChange({ ...config, session: { ...session, ...patch } });

  return (
    <SettingsPanel title={t("Session title", "会话标题")} description={t("Name a new session after its first reply; manual names are kept.", "首次回复后自动命名一次，保留手动修改的名称。")}>
      <FieldGrid>
        <SwitchField label={t("Auto title on first turn", "首轮自动标题")} hint={t("Generate a title after the first assistant reply.", "首次助手回复后生成会话标题。")} checked={enabled} anchor="runtime.session.auto_title_enabled" configKey="session.auto_title_enabled" onChange={(value) => update({ auto_title_enabled: value })} />
        <SettingsField label={t("Title model", "标题模型")} hint={t("Leave empty to follow the current conversation model.", "留空则跟随当前会话模型。")} anchor="runtime.session.auto_title_model" configKey="session.auto_title_model">
          <ModelChoiceSelect config={config} providerId={session.auto_title_provider_id} model={session.auto_title_model} disabled={!enabled} inheritLabel={t("Session model", "会话模型")} onChange={(providerId, model) => update({ auto_title_provider_id: providerId || undefined, auto_title_model: model || undefined })} />
        </SettingsField>
      </FieldGrid>
    </SettingsPanel>
  );
}
