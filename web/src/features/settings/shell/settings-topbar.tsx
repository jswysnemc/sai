import { useI18n } from "../../i18n/use-i18n";

/**
 * 渲染设置页顶栏：仅保留桌面窗口拖拽条。
 *
 * 分区标题已移入内容区页头，避免与侧栏、页内标题重复命名。
 * 全局保存操作仍在主内容区底部浮动栏。
 *
 * @returns 顶栏拖拽条
 */
export function SettingsTopbar() {
  const { t } = useI18n();
  return (
    <header className="settings-topbar" aria-label={t("Settings", "设置")}>
      <div className="settings-topbar-inner" />
    </header>
  );
}
