import { FolderGit2, GitBranch, PanelLeft, PanelRightOpen } from "lucide-react";
import type { Workspace } from "../../api/contracts";
import { localizeApiMessage } from "../../api/api-error";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import { MOBILE_SIDEBAR_TOGGLE_EVENT } from "../workspace/mobile-workbench-state";
import { OPEN_WORKSPACE_SIDEBAR_EVENT } from "../workspace/workspace-passive-diff";
import "./chat-session-header.css";

type ChatSessionHeaderProps = {
  title: string;
  workspace?: Pick<Workspace, "name" | "path"> | null;
  branch?: string;
};

/**
 * 渲染会话标题、项目上下文和移动端工作区操作。
 *
 * @param props 会话标题、当前工作区和 Git 分支
 * @returns 聊天页面头部
 */
export function ChatSessionHeader({ title, workspace, branch }: ChatSessionHeaderProps) {
  const { locale, t } = useI18n();
  const workspaceName = workspace ? localizeApiMessage(workspace.name, locale) : "";
  const hasContext = Boolean(workspace?.path || branch);
  return (
    <header className="chat-header">
      <Button
        className="chat-header-menu"
        onClick={() => window.dispatchEvent(new Event(MOBILE_SIDEBAR_TOGGLE_EVENT))}
        aria-label={t("Open session sidebar", "打开会话侧栏")}
        title={t("Open session sidebar", "打开会话侧栏")}
      >
        <PanelLeft size={16} aria-hidden />
      </Button>
      <div className="chat-header-main">
        <h1 title={title}>{title}</h1>
        {hasContext && (
          <div className="chat-header-context" aria-label={t("Project context", "项目上下文")}>
            {workspace?.path && (
              <span
                className="chat-header-context-item chat-header-workspace"
                title={workspace.path}
                aria-label={t(
                  `Project ${workspaceName}, path ${workspace.path}`,
                  `项目 ${workspaceName}，路径 ${workspace.path}`
                )}
              >
                <FolderGit2 size={12} aria-hidden />
                <span className="chat-header-path">{workspace.path}</span>
                <span className="chat-header-workspace-name">{workspaceName}</span>
              </span>
            )}
            {branch && (
              <span
                className="chat-header-context-item chat-header-branch"
                title={t(`Git branch: ${branch}`, `Git 分支：${branch}`)}
                aria-label={t(`Git branch: ${branch}`, `Git 分支：${branch}`)}
              >
                <GitBranch size={12} aria-hidden />
                <span>{branch}</span>
              </span>
            )}
          </div>
        )}
      </div>
      <div className="chat-header-panel">
        <Button
          className="chat-header-plus"
          onClick={() => window.dispatchEvent(new Event(OPEN_WORKSPACE_SIDEBAR_EVENT))}
          aria-label={t("Open side panel", "打开右侧栏")}
          title={t("Open side panel", "打开右侧栏")}
        >
          <PanelRightOpen size={16} aria-hidden />
        </Button>
      </div>
    </header>
  );
}
