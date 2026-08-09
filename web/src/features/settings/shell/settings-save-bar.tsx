import { Save } from "lucide-react";
import { SaveStatusBadge } from "../save-status-badge";
import { showsAppConfigSave } from "../settings-registry";
import type { SettingsSectionMeta } from "../settings-types";
import { useI18n } from "../../i18n/use-i18n";

type SettingsSaveBarProps = {
  meta: SettingsSectionMeta | undefined;
  dirty: boolean;
  saving: boolean;
  saveError: boolean;
  loaded: boolean;
  onSave: () => void;
};

/**
 * 渲染顶栏保存区。
 *
 * 是否展示全局保存完全由注册表的 appConfig 参与方式派生：
 * required 常驻，optional 仅在有待保存修改时露出，其余展示分区保存提示。
 *
 * @param props 分区元数据与全局 AppConfig 保存状态
 * @returns 保存徽标与按钮，或分区保存提示
 */
export function SettingsSaveBar({
  meta,
  dirty,
  saving,
  saveError,
  loaded,
  onSave
}: SettingsSaveBarProps) {
  const { t } = useI18n();
  const use = meta?.appConfig ?? "required";

  // 1. 参与全局 AppConfig 的面：展示徽标 + Save
  if (showsAppConfigSave(use, dirty)) {
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

  // 2. 其余面：展示注册表声明的保存提示
  if (meta?.saveHintEn && meta.saveHintZh) {
    return (
      <span className="settings-save-hint">
        {t(meta.saveHintEn, meta.saveHintZh)}
      </span>
    );
  }
  return null;
}
