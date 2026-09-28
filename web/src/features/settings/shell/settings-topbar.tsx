import { SettingsSaveBar } from "./settings-save-bar";
import type { SettingsSectionMeta } from "../settings-types";
import { useI18n } from "../../i18n/use-i18n";

type SettingsTopbarProps = {
  meta: SettingsSectionMeta | undefined;
  subview?: string;
  dirty: boolean;
  saving: boolean;
  saveErrorMessage?: string;
  loaded: boolean;
  validationError?: string | null;
  onSave: () => void;
  onDiscard: () => void;
};

/**
 * 渲染设置页顶栏：分区与子页路径在左，保存区在右。
 *
 * 分区名只在这里出现一次，页面内容不再重复分区标题。
 *
 * @param props 分区元数据、当前子页与保存状态
 * @returns 顶栏
 */
export function SettingsTopbar({ meta, subview, dirty, saving, saveErrorMessage, loaded, validationError, onSave, onDiscard }: SettingsTopbarProps) {
  const { t } = useI18n();
  const Icon = meta?.icon;
  const subviewMeta = meta?.subviews?.find((item) => item.id === subview);
  return (
    <header className="settings-topbar">
      <div className="settings-topbar-inner">
        <div className="settings-crumbs">
          {Icon && <Icon size={14} />}
          <h1>{meta ? t(meta.labelEn, meta.labelZh) : t("Settings", "设置")}</h1>
          {subviewMeta && (
            <>
              <span className="settings-crumb-separator" aria-hidden="true">/</span>
              <span className="settings-crumb-subview">{t(subviewMeta.labelEn, subviewMeta.labelZh)}</span>
            </>
          )}
        </div>
        <div className="settings-topbar-actions">
          <SettingsSaveBar
            meta={meta}
            dirty={dirty}
            saving={saving}
            saveErrorMessage={saveErrorMessage}
            loaded={loaded}
            validationError={validationError}
            onSave={onSave}
            onDiscard={onDiscard}
          />
        </div>
      </div>
    </header>
  );
}
