import { MoreHorizontal } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { executeGitCommand, type GitCommandId } from "../commands/git-command-registry";
import { RepositoryResources } from "../resources/repository-resources";
import type { GitOperationUiOptions, RunGitOperation } from "../types";

type MoreActionsMenuProps = {
  busy: boolean;
  dirtyTotal: number;
  repoRoot: string | null;
  confirmSync: boolean;
  confirmForcePush: boolean;
  runOperation: RunGitOperation;
};

/**
 * 渲染 Source Control 的远端和 stash 扩展操作菜单。
 *
 * @param props 仓库计数、忙碌状态和操作回调
 * @returns 分组操作菜单
 */
export function MoreActionsMenu(props: MoreActionsMenuProps) {
  const { t } = useI18n();
  const rootRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (!open) return;
    /** 点击菜单外部时关闭操作菜单。 */
    const closeOutside = (event: PointerEvent) => {
      if (event.target instanceof Element && event.target.closest(".ui-modal")) return;
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", closeOutside);
    return () => document.removeEventListener("pointerdown", closeOutside);
  }, [open]);

  /**
   * 执行菜单动作并关闭菜单。
   *
   * @param commandId VS Code 风格 Git 命令标识
   * @param options 可选确认信息
   * @returns 无返回值
   */
  const run = async (commandId: GitCommandId, options: GitOperationUiOptions = {}) => {
    setOpen(false);
    await executeGitCommand(commandId, props.runOperation, options);
  };

  return (
    <div className="git-more-actions" ref={rootRef}>
      <Button
        className="git-toolbar-icon"
        disabled={props.busy}
        onClick={() => setOpen((value) => !value)}
        aria-expanded={open}
        aria-label={t("More Git actions", "更多 Git 操作")}
        title={t("More actions", "更多操作")}
      >
        <MoreHorizontal size={14} />
      </Button>
      {open && (
        <div className="git-more-actions-menu" role="menu">
          <span>{t("Remote", "远端")}</span>
          <Button onClick={() => void run("git.pullRebase")}>{t("Pull (Rebase)", "拉取并变基")}</Button>
          <Button onClick={() => void run("git.sync", props.confirmSync ? {
            confirmTitle: t("Sync changes?", "同步改动？"),
            confirmDescription: t("Git will pull remote changes and then push the current branch.", "Git 将先获取远端改动，再推送当前分支。")
          } : {})}>{t("Sync", "同步")}</Button>
          <Button onClick={() => void run("git.pushForce", {
            ...(props.confirmForcePush ? {
              confirmTitle: t("Force push with lease?", "使用租约强制推送？"),
              confirmDescription: t("Remote commits may be replaced. Git will refuse if the remote changed unexpectedly.", "远端提交可能被替换；远端状态意外变化时 Git 会拒绝执行。")
            } : {})
          })}>{t("Force Push with Lease", "使用租约强制推送")}</Button>
          <span>{t("Stash", "储藏")}</span>
          <Button disabled={props.dirtyTotal === 0} onClick={() => void run("git.stash", { message: "Sai stash" })}>
            {t("Stash", "储藏修改")}
          </Button>
          <Button disabled={props.dirtyTotal === 0} onClick={() => void run("git.stash", { message: "Sai stash", include_untracked: true })}>
            {t("Stash Including Untracked", "储藏并包含未跟踪文件")}
          </Button>
          <RepositoryResources repoRoot={props.repoRoot} open={open} busy={props.busy} runOperation={props.runOperation} />
        </div>
      )}
    </div>
  );
}
