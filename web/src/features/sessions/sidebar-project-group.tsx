import { ChevronDown, FolderOpen, MoreHorizontal, Plus, X } from "lucide-react";
import { useEffect, useId, useState, type ReactNode } from "react";
import type { WorkspaceSessions } from "../../api/contracts";
import { localizeApiMessage } from "../../api/api-error";
import { Button } from "../../shared/ui/button/button";
import { ActionMenu } from "../../shared/ui/menu/action-menu";
import { useI18n } from "../i18n/use-i18n";
import { SessionWorkspaceIcon } from "./session-workspace-icon";
import "./sidebar-project-group.css";

type SidebarProjectGroupProps = {
  workspace: WorkspaceSessions;
  children: ReactNode;
  pending: boolean;
  canClose: boolean;
  onCreate: () => void;
  onOpen: () => void;
  onClose: () => void;
};

/**
 * 按项目组织会话，持久化折叠状态，并保留项目级操作入口。
 * @param props 项目、会话列表及创建、切换和关闭回调
 * @returns 可折叠的项目会话分组
 */
export function SidebarProjectGroup({ workspace, children, pending, canClose, onCreate, onOpen, onClose }: SidebarProjectGroupProps) {
  const { locale, t } = useI18n();
  const storageKey = `sai.sidebar.project.${workspace.workspace_id}`;
  const [open, setOpen] = useState(() => window.localStorage.getItem(storageKey) !== "false");
  const id = useId();
  const activeSessionId = workspace.sessions.find((session) => session.active)?.id;
  const name = localizeApiMessage(workspace.workspace_name, locale);
  useEffect(() => {
    if (workspace.active) setOpen(true);
  }, [workspace.active, activeSessionId]);
  useEffect(() => {
    window.localStorage.setItem(storageKey, String(open));
  }, [open, storageKey]);

  return (
    <section className={`sidebar-project-group${workspace.active ? " is-current" : ""}`} aria-label={name}>
      <div className="sidebar-project-heading">
        <Button variant="ghost" className="sidebar-project-toggle" aria-expanded={open} aria-controls={id} onClick={() => setOpen((value) => !value)} title={workspace.workspace_path}>
          <ChevronDown size={12} className={open ? "" : "is-collapsed"} />
          <SessionWorkspaceIcon isGitRepository={workspace.is_git_repository} size={14} />
          <span>{name}</span>
        </Button>
        <div className="sidebar-project-actions">
          <Button variant="ghost" size="icon" disabled={pending} onClick={onCreate} aria-label={t(`New task in ${name}`, `在 ${name} 新建任务`)} title={t("New task", "新建任务")}><Plus size={14} /></Button>
          <ActionMenu label={t(`Manage project ${name}`, `管理项目 ${name}`)} trigger={<MoreHorizontal size={14} />} items={[
            { id: "open", label: t("Open workspace", "打开工作区"), icon: <FolderOpen size={14} />, onSelect: onOpen },
            { id: "new", label: t("New task", "新建任务"), icon: <Plus size={14} />, disabled: pending, onSelect: onCreate },
            { id: "close", label: t("Close workspace", "关闭工作区"), icon: <X size={14} />, disabled: !canClose, separator: true, onSelect: onClose }
          ]} />
        </div>
      </div>
      <div id={id} hidden={!open} className="sidebar-project-sessions">{children}</div>
    </section>
  );
}
