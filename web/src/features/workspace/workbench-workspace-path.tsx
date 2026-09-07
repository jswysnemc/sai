import { useQuery } from "@tanstack/react-query";
import { FolderGit2 } from "lucide-react";
import { api } from "../../api/client";
import { useI18n } from "../i18n/use-i18n";

/**
 * 【工作台】【路径展示】直接展示当前工作区的完整路径，长路径可横向滚动。
 * @returns 不含切换操作的工作区路径
 */
export function WorkbenchWorkspacePath() {
  const { t } = useI18n();
  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: api.workspaces.list });
  const active = workspaces.data?.workspaces.find((workspace) => workspace.id === workspaces.data.active_id);
  return (
    <div className="workbench-workspace-path min-w-0" aria-label={t("Current workspace path", "当前工作区路径")} title={active?.path}>
      <FolderGit2 size={13} aria-hidden="true" />
      <span dir="ltr">{active?.path ?? t("Workspace", "工作区")}</span>
    </div>
  );
}
