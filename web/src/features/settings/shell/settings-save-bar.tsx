import { Save } from "lucide-react";
import { SaveStatusBadge } from "../save-status-badge";
import { showsGlobalAppConfigSave } from "../settings-registry";
import type { SettingsSectionId, SettingsSurfaceKind } from "../settings-types";
import { useI18n } from "../../i18n/use-i18n";

type SettingsSaveBarProps = {
  kind: SettingsSurfaceKind;
  sectionId: SettingsSectionId;
  dirty: boolean;
  saving: boolean;
  saveError: boolean;
  loaded: boolean;
  onSave: () => void;
};

/**
 * 按 section 面类型渲染顶栏保存区。
 *
 * @param props 面类型与全局 AppConfig 保存状态
 * @returns 保存徽标与按钮，或独立配置/只读提示
 */
export function SettingsSaveBar({
  kind,
  sectionId,
  dirty,
  saving,
  saveError,
  loaded,
  onSave
}: SettingsSaveBarProps) {
  const { t } = useI18n();

  // Skills / Memory 页可能改写 AppConfig（skills 行为、plugins.memory）；脏时露出全局 Save
  const showAppConfigSave =
    showsGlobalAppConfigSave(kind)
    || ((sectionId === "skills" || sectionId === "memory") && dirty);

  // 1. 全局 AppConfig：展示徽标 + Save
  if (showAppConfigSave) {
    return (
      <>
        <SaveStatusBadge dirty={dirty} saving={saving} saveError={saveError} loaded={loaded} />
        <button
          type="button"
          className="settings-save"
          onClick={onSave}
          disabled={!loaded || !dirty || saving}
        >
          <Save size={14} />
          {saving ? t("Saving", "正在保存") : t("Save changes", "保存修改")}
        </button>
      </>
    );
  }

  // 2. 独立配置文档：提示在本节内保存
  if (kind === "local-config") {
    return (
      <span className="settings-save-hint" title={t("This section saves separately", "本节使用独立保存")}>
        {t("Saves in section", "在本节内保存")}
      </span>
    );
  }

  // 3. 浏览器偏好：即时生效
  if (kind === "client-pref") {
    return (
      <span className="settings-save-hint">
        {t("Applies immediately", "即时生效")}
      </span>
    );
  }

  // 4. 运维 / 统计：无全局保存
  return (
    <span className="settings-save-hint">
      {kind === "analytics"
        ? t("Read only", "只读")
        : t("Actions in section", "操作在本节内完成")}
    </span>
  );
}
