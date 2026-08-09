import { FolderOpen, PanelLeftOpen, Plus, Settings } from "lucide-react";
import type { RefObject } from "react";
import { SaiLogo } from "../../shared/ui/sai-logo";
import { LocaleSwitcher } from "../i18n/locale-switcher";
import { useI18n } from "../i18n/use-i18n";
import { SidebarAppMenu } from "./sidebar-app-menu";

type SidebarCollapsedRailProps = {
  onExpand: () => void;
  onNewSession: () => void;
  newSessionPending: boolean;
  onOpenDirectory: () => void;
  /** 设置菜单是否展开 */
  appMenuOpen: boolean;
  onToggleAppMenu: () => void;
  onCloseAppMenu: () => void;
  /** 设置菜单是否命中当前路由 */
  appMenuActive: boolean;
  /** 外部点击关闭用的容器引用 */
  appMenuRef: RefObject<HTMLDivElement | null>;
  onAfterNavigate?: () => void;
};

/**
 * 渲染折叠态的侧栏图标竖条。
 *
 * 竖条只保留四类入口：展开、打开目录、新建会话与设置菜单，
 * 与展开态的信息架构一一对应，折叠不改变功能集合。
 *
 * @param props 各入口回调与设置菜单状态
 * @returns 折叠态竖条
 */
export function SidebarCollapsedRail({
  onExpand,
  onNewSession,
  newSessionPending,
  onOpenDirectory,
  appMenuOpen,
  onToggleAppMenu,
  onCloseAppMenu,
  appMenuActive,
  appMenuRef,
  onAfterNavigate
}: SidebarCollapsedRailProps) {
  const { t } = useI18n();
  return (
    <>
      <button type="button" className="sidebar-rail-button brand-rail" onClick={onExpand} aria-label={t("Expand session sidebar", "展开会话侧栏")} title={t("Expand session sidebar", "展开会话侧栏")}>
        <SaiLogo size={18} />
      </button>
      <button type="button" className="sidebar-rail-button" onClick={onExpand} aria-label={t("Expand session sidebar", "展开会话侧栏")} title={t("Expand session sidebar", "展开会话侧栏")}>
        <PanelLeftOpen size={17} />
      </button>
      <button type="button" className="sidebar-rail-button" onClick={onOpenDirectory} aria-label={t("Open server directory", "打开服务端目录")} title={t("Open server directory", "打开服务端目录")}>
        <FolderOpen size={17} />
      </button>
      <button type="button" className="sidebar-rail-button" onClick={onNewSession} disabled={newSessionPending} aria-label={t("New session", "新建会话")} title={t("New session", "新建会话")}>
        <Plus size={17} />
      </button>
      <div className="sidebar-app-menu collapsed-app-menu" ref={appMenuRef}>
        <button
          type="button"
          className={`sidebar-rail-button${appMenuOpen || appMenuActive ? " active" : ""}`}
          onClick={onToggleAppMenu}
          aria-label={t("Settings menu", "设置菜单")}
          title={t("Settings menu", "设置菜单")}
          aria-expanded={appMenuOpen}
        >
          <Settings size={17} strokeWidth={1.8} />
        </button>
        <LocaleSwitcher compact />
        {appMenuOpen && (
          <SidebarAppMenu
            className="rail"
            onClose={onCloseAppMenu}
            onOpenDirectory={onOpenDirectory}
            onAfterNavigate={onAfterNavigate}
          />
        )}
      </div>
    </>
  );
}
