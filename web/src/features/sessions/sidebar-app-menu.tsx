import { Cable, CalendarClock, FolderOpen, Settings, Sparkles } from "lucide-react";
import { NavLink } from "react-router-dom";
import { useI18n } from "../i18n/use-i18n";

type SidebarAppMenuProps = {
  /** 附加类名；折叠态竖条用 rail 变体定位 */
  className?: string;
  /** 选择任意条目后关闭菜单 */
  onClose: () => void;
  /** 打开服务端目录对话框 */
  onOpenDirectory: () => void;
  /** 路由跳转完成后的回调（如关闭移动端抽屉） */
  onAfterNavigate?: () => void;
};

/**
 * 渲染设置弹出菜单：服务端目录、设置、网关、定时任务与技能。
 *
 * 展开页脚与折叠竖条共用这一份条目列表，入口只在这里维护一处；
 * 定时任务与技能原先散在会话操作条上，收进来后侧栏中部只留会话本身。
 *
 * @param props 关闭回调、目录对话框与导航后回调
 * @returns 弹出菜单
 */
export function SidebarAppMenu({ className, onClose, onOpenDirectory, onAfterNavigate }: SidebarAppMenuProps) {
  const { t } = useI18n();
  /** 条目点击后的公共收尾：关菜单、通知外层收抽屉 */
  const finish = () => {
    onClose();
    onAfterNavigate?.();
  };

  return (
    <div className={className ? `sidebar-app-popover ${className}` : "sidebar-app-popover"}>
      <button type="button" onClick={() => { onClose(); onOpenDirectory(); }}>
        <FolderOpen size={14} /><span>{t("Open server directory", "打开服务端目录")}</span>
      </button>
      <NavLink to="/settings" end onClick={finish} className={({ isActive }) => isActive ? "active" : ""}>
        <Settings size={14} /><span>{t("Settings", "设置")}</span>
      </NavLink>
      <NavLink to="/gateways" onClick={finish} className={({ isActive }) => isActive ? "active" : ""}>
        <Cable size={14} /><span>{t("Gateways", "网关")}</span>
      </NavLink>
      <NavLink to="/cron-jobs" onClick={finish} className={({ isActive }) => isActive ? "active" : ""}>
        <CalendarClock size={14} /><span>{t("Scheduled tasks", "定时任务")}</span>
      </NavLink>
      <NavLink to="/settings/skills" onClick={finish} className={({ isActive }) => isActive ? "active" : ""}>
        <Sparkles size={14} /><span>{t("Skills", "技能")}</span>
      </NavLink>
    </div>
  );
}
