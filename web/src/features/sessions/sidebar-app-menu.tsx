import { Cable, CalendarClock, FolderOpen, Settings, Sparkles } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { ActionMenu } from "../../shared/ui/menu/action-menu";
import { useI18n } from "../i18n/use-i18n";

type SidebarAppMenuProps = {
  collapsed?: boolean;
  onOpenDirectory: () => void;
  onAfterNavigate?: () => void;
};

/**
 * 统一侧栏设置入口，展开和折叠布局共享菜单操作与键盘行为。
 * @param props 侧栏尺寸、目录操作和导航完成回调
 * @returns 设置菜单和触发按钮
 */
export function SidebarAppMenu({ collapsed = false, onOpenDirectory, onAfterNavigate }: SidebarAppMenuProps) {
  const { t } = useI18n();
  const navigate = useNavigate();
  /**
   * 前往设置分区并结束当前导航。
   * @param path 目标应用路由
   * @returns 无返回值
   */
  const openPage = (path: string) => {
    navigate(path);
    onAfterNavigate?.();
  };
  return (
    <ActionMenu
      className={collapsed ? "" : "sidebar-settings-menu"}
      triggerClassName={collapsed ? "sidebar-rail-button" : "sidebar-settings-link"}
      label={t("Settings", "设置")}
      trigger={<><Settings size={15} />{!collapsed && <span>{t("Settings", "设置")}</span>}</>}
      items={[
        { id: "settings", label: t("Settings", "设置"), icon: <Settings size={15} />, onSelect: () => openPage("/settings") },
        { id: "skills", label: t("Skills", "技能"), icon: <Sparkles size={15} />, onSelect: () => openPage("/settings/skills") },
        { id: "cron", label: t("Scheduled tasks", "定时任务"), icon: <CalendarClock size={15} />, onSelect: () => openPage("/cron-jobs") },
        { id: "gateways", label: t("Gateways", "网关"), icon: <Cable size={15} />, onSelect: () => openPage("/gateways") },
        { id: "directory", label: t("Open server directory", "打开服务端目录"), icon: <FolderOpen size={15} />, separator: true, onSelect: onOpenDirectory }
      ]}
    />
  );
}
