import { Plus, Search } from "lucide-react";
import { modKeyLabel } from "../../shared/mod-key";
import { useI18n } from "../i18n/use-i18n";

type SessionSidebarActionsProps = {
  onNewSession: () => void;
  onSearch: () => void;
  createPending?: boolean;
};

/**
 * 渲染会话视图顶部的常用入口。
 *
 * 只保留新建与搜索两个高频动作；定时任务、技能属于应用级导航，
 * 已归入页脚的设置菜单，侧栏中部留给会话本身。
 *
 * @param props 新建与搜索回调
 * @returns 紧凑操作列表
 */
export function SessionSidebarActions({ onNewSession, onSearch, createPending = false }: SessionSidebarActionsProps) {
  const { t } = useI18n();
  const modifier = modKeyLabel();
  return (
    <div className="sidebar-session-actions" role="toolbar" aria-label={t("Session actions", "会话操作")}>
      <button type="button" onClick={onNewSession} disabled={createPending}>
        <Plus size={14} /><span>{createPending ? t("Creating", "正在创建") : t("New task", "新建任务")}</span>
      </button>
      <button type="button" onClick={onSearch}><Search size={14} /><span>{t("Search", "搜索")}</span><kbd>{modifier}+K</kbd></button>
    </div>
  );
}
