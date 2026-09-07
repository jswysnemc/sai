import { Files, FolderGit2, MessagesSquare } from "lucide-react";
import { SegmentedControl } from "../../shared/ui/segmented-control";
import { useI18n } from "../i18n/use-i18n";
import { SessionScopeControl, type SessionScope } from "./session-scope-control";
import "./sidebar-navigation.css";

export type SidebarView = "sessions" | "workspaces" | "files";

type SidebarNavigationProps = {
  view: SidebarView;
  onViewChange: (view: SidebarView) => void;
  scope: SessionScope;
  onScopeChange: (scope: SessionScope) => void;
};

/**
 * 【会话侧栏】【导航】将视图切换与会话范围放在同一行，保留各自状态和键盘操作。
 * @param props 当前视图、会话范围与对应切换回调
 * @returns 单行侧栏导航
 */
export function SidebarNavigation({ view, onViewChange, scope, onScopeChange }: SidebarNavigationProps) {
  const { t } = useI18n();
  return (
    <div className="sidebar-navigation gap-1 max-sm:gap-0.5">
      <SegmentedControl className="sidebar-view-switcher" value={view} onChange={onViewChange} ariaLabel={t("Sidebar view", "侧栏视图")} options={[
        { value: "sessions", label: t("Sessions", "会话"), title: t("Sessions", "会话"), icon: <MessagesSquare size={14} aria-hidden="true" /> },
        { value: "workspaces", label: t("Workspaces", "工作区"), title: t("Workspaces", "工作区"), icon: <FolderGit2 size={14} aria-hidden="true" /> },
        { value: "files", label: t("Files", "文件"), title: t("Files", "文件"), icon: <Files size={14} aria-hidden="true" /> }
      ]} />
      {view === "sessions" && <SessionScopeControl value={scope} onChange={onScopeChange} />}
    </div>
  );
}
