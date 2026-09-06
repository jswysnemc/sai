import { Blocks, CalendarClock, Search, SquarePen } from "lucide-react";
import { Button } from "../../shared/ui/button/button";
import { modKeyLabel } from "../../shared/mod-key";
import { useI18n } from "../i18n/use-i18n";

type SessionSidebarActionsProps = {
  onNewSession: () => void;
  onSearch: () => void;
  onScheduledTasks: () => void;
  onSkills: () => void;
  createPending?: boolean;
};

/**
 * 提供新任务、搜索、定时任务与技能的常用导航入口。
 * @param props 导航回调和创建状态
 * @returns 侧栏常用操作列表
 */
export function SessionSidebarActions({ onNewSession, onSearch, onScheduledTasks, onSkills, createPending = false }: SessionSidebarActionsProps) {
  const { t } = useI18n();
  const modifier = modKeyLabel();
  return (
    <div className="sidebar-session-actions" role="toolbar" aria-label={t("Session actions", "会话操作")}>
      <Button variant="ghost" onClick={onNewSession} disabled={createPending} title={`${modifier}+Shift+O`}><SquarePen size={15} /><span>{createPending ? t("Creating", "正在创建") : t("New task", "新建任务")}</span><kbd>{modifier}+Shift+O</kbd></Button>
      <Button variant="ghost" onClick={onSearch}><Search size={15} /><span>{t("Search", "搜索")}</span><kbd>{modifier}+K</kbd></Button>
      <Button variant="ghost" onClick={onScheduledTasks}><CalendarClock size={15} /><span>{t("Scheduled tasks", "定时任务")}</span></Button>
      <Button variant="ghost" onClick={onSkills}><Blocks size={15} /><span>{t("Skills", "技能")}</span></Button>
    </div>
  );
}
