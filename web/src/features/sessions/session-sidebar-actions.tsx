import { CalendarClock, Plus, Search, Sparkles } from "lucide-react";
import { useI18n } from "../i18n/use-i18n";

type SessionSidebarActionsProps = {
  onNewSession: () => void;
  onSearch: () => void;
  onScheduledTasks: () => void;
  onSkills: () => void;
};

/**
 * 渲染会话视图顶部的常用入口。
 *
 * @param props 新建、搜索、定时任务和技能入口回调
 * @returns 紧凑操作列表
 */
export function SessionSidebarActions({ onNewSession, onSearch, onScheduledTasks, onSkills }: SessionSidebarActionsProps) {
  const { t } = useI18n();
  return (
    <div className="sidebar-session-actions" role="toolbar" aria-label={t("Session actions", "会话操作")}>
      <button type="button" onClick={onNewSession}><Plus size={14} /><span>{t("New task", "新建任务")}</span></button>
      <button type="button" onClick={onSearch}><Search size={14} /><span>{t("Search", "搜索")}</span><kbd>Ctrl+K</kbd></button>
      <button type="button" onClick={onScheduledTasks}><CalendarClock size={14} /><span>{t("Scheduled tasks", "定时任务")}</span></button>
      <button type="button" onClick={onSkills}><Sparkles size={14} /><span>{t("Skills", "技能")}</span></button>
    </div>
  );
}
