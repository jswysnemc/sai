import { FolderGit2, PanelLeft } from "lucide-react";
import type { ReactNode } from "react";
import type { Workspace } from "../../api/contracts";
import { localizeApiMessage } from "../../api/api-error";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import { MOBILE_SIDEBAR_TOGGLE_EVENT } from "../workspace/mobile-workbench-state";
import "./chat-session-header.css";

type ChatSessionHeaderProps = {
  title: string;
  workspace?: Pick<Workspace, "name" | "path"> | null;
  branch?: string;
  viewSwitch?: ReactNode;
  branchNavigation?: ReactNode;
  actions?: ReactNode;
};

/**
 * 渲染项目面包屑、会话标题和统一工作台操作。
 * @param props 当前项目、标题、视图切换与工具栏
 * @returns 固定在会话顶部的标题栏
 */
export function ChatSessionHeader({ title, workspace, branch, viewSwitch, branchNavigation, actions }: ChatSessionHeaderProps) {
  const { locale, t } = useI18n();
  const workspaceName = workspace ? localizeApiMessage(workspace.name, locale) : "";
  return (
    <header className="chat-header">
      <Button variant="ghost" size="icon" className="chat-header-menu md:hidden" onClick={() => window.dispatchEvent(new Event(MOBILE_SIDEBAR_TOGGLE_EVENT))} aria-label={t("Open session sidebar", "打开会话侧栏")} title={t("Open session sidebar", "打开会话侧栏")}><PanelLeft size={17} /></Button>
      <div className="chat-header-main">
        {workspace && <span className="chat-header-project hidden lg:inline-flex" aria-label={t("Project context", "项目上下文")} title={[workspace.path, branch].filter(Boolean).join(" · ")}><FolderGit2 size={13} aria-hidden /><span>{workspaceName}</span><span className="chat-header-divider" aria-hidden>/</span></span>}
        <h1 title={title}>{title}</h1>
      </div>
      {branchNavigation}
      {viewSwitch && <div className="chat-header-view-switch">{viewSwitch}</div>}
      {actions}
    </header>
  );
}
