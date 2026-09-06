import { CalendarClock, FolderOpen, PanelLeftOpen, Search, Settings2, SquarePen } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { Button } from "../../shared/ui/button/button";
import { LocaleSwitcher } from "../i18n/locale-switcher";
import { useI18n } from "../i18n/use-i18n";
import { SidebarAppMenu } from "./sidebar-app-menu";

type SidebarCollapsedRailProps = {
  onExpand: () => void;
  onNewSession: () => void;
  onSearch: () => void;
  newSessionPending: boolean;
  onOpenDirectory: () => void;
  onAfterNavigate?: () => void;
};

/**
 * 渲染紧凑导航竖栏，保留新任务、搜索、定时任务和设置入口。
 * @param props 导航操作和新任务创建状态
 * @returns 折叠侧栏
 */
export function SidebarCollapsedRail({ onExpand, onNewSession, onSearch, newSessionPending, onOpenDirectory, onAfterNavigate }: SidebarCollapsedRailProps) {
  const { t } = useI18n();
  const navigate = useNavigate();
  const shortcuts = [
    { label: t("Expand session sidebar", "展开会话侧栏"), icon: PanelLeftOpen, action: onExpand },
    { label: t("New task", "新建任务"), icon: SquarePen, action: onNewSession, disabled: newSessionPending },
    { label: t("Search", "搜索"), icon: Search, action: onSearch },
    { label: t("Scheduled tasks", "定时任务"), icon: CalendarClock, action: () => { navigate("/cron-jobs"); onAfterNavigate?.(); } },
    { label: t("Skills", "技能"), icon: Settings2, action: () => { navigate("/settings/skills"); onAfterNavigate?.(); } },
    { label: t("Open server directory", "打开服务端目录"), icon: FolderOpen, action: onOpenDirectory }
  ];
  return (
    <>
      {shortcuts.map(({ label, icon: Icon, action, disabled }) => (
        <Button key={label} variant="ghost" size="icon" className="sidebar-rail-button" onClick={action} disabled={disabled} aria-label={label} title={label}><Icon size={17} /></Button>
      ))}
      <div className="collapsed-app-menu">
        <SidebarAppMenu collapsed onOpenDirectory={onOpenDirectory} onAfterNavigate={onAfterNavigate} />
        <LocaleSwitcher compact />
      </div>
    </>
  );
}
