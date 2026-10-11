import type { SettingsSectionMeta } from "../settings-types";
import { useI18n } from "../../i18n/use-i18n";

type SettingsTopbarProps = {
  meta: SettingsSectionMeta | undefined;
  subview?: string;
};

/**
 * 渲染设置页顶栏：仅保留分区与子页面包屑路径及轻量保存提示。
 *
 * 全局保存操作已下沉至主内容区浮动栏，顶栏右侧保持清爽，
 * 避免在桌面客户端中与右上角窗口控制按钮挤在一起或发生误触。
 *
 * @param props 分区元数据与当前子页标识
 * @returns 顶栏组件
 */
export function SettingsTopbar({ meta, subview }: SettingsTopbarProps) {
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
        {meta?.saveHintEn && meta?.saveHintZh && (
          <div className="settings-topbar-actions">
            <span className="settings-save-hint">{t(meta.saveHintEn, meta.saveHintZh)}</span>
          </div>
        )}
      </div>
    </header>
  );
}
