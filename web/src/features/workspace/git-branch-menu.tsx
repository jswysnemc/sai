import { ArrowDown, ArrowUp, Check, ChevronDown, GitBranch, GitMerge, GitPullRequest, Pencil, Plus, Trash2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { GitOperationAction, GitOperationOptions } from "../../api/git-contracts";
import type { GitBranch as GitBranchInfo, GitOperationResponse, GitRepositoryState } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { Modal } from "../../shared/ui/dialog/modal";
import { useI18n } from "../i18n/use-i18n";
import { createBranchNameSuggestion } from "../source-control/branches/branch-name-suggestion";

type GitBranchMenuProps = {
  state: GitRepositoryState;
  branches: GitBranchInfo[];
  loading: boolean;
  open: boolean;
  busy: boolean;
  suggestBranchNames: boolean;
  onOpenChange: (open: boolean) => void;
  onOperation: (action: GitOperationAction, options?: GitOperationOptions) => Promise<GitOperationResponse | undefined>;
};

/**
 * 渲染 Git 分支选择、创建、重命名和删除入口。
 *
 * @param props 仓库状态、分支数据和操作回调
 * @returns 分支菜单
 */
export function GitBranchMenu(props: GitBranchMenuProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const rootRef = useRef<HTMLDivElement>(null);
  const [createName, setCreateName] = useState("");
  const [renameBranch, setRenameBranch] = useState<GitBranchInfo | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const localBranches = props.branches.filter((branch) => branch.kind === "local");
  const remoteBranches = props.branches.filter((branch) => branch.kind === "remote");

  useEffect(() => {
    if (!props.open) return;
    const handlePointerDown = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) props.onOpenChange(false);
    };
    document.addEventListener("pointerdown", handlePointerDown);
    return () => document.removeEventListener("pointerdown", handlePointerDown);
  }, [props.open, props.onOpenChange]);

  useEffect(() => {
    if (!props.open || !props.suggestBranchNames) return;
    setCreateName((current) => current || createBranchNameSuggestion());
  }, [props.open, props.suggestBranchNames]);

  /**
   * 创建并切换到新分支。
   *
   * @returns 操作完成后的 Promise
   */
  const create = async () => {
    const branch = createName.trim();
    if (!branch) return;
    const result = await props.onOperation("create_branch", { branch });
    if (!result?.ok) return;
    setCreateName("");
    props.onOpenChange(false);
  };

  /**
   * 切换所选本地或远端分支。
   *
   * @param branch 所选分支
   * @returns 操作完成后的 Promise
   */
  const select = async (branch: GitBranchInfo) => {
    if (branch.current) return;
    const result = await props.onOperation("switch_branch", {
      branch: branch.full_name,
      branch_kind: branch.kind === "remote" ? "remote" : "local"
    });
    if (result?.ok) props.onOpenChange(false);
  };

  /**
   * 打开本地分支重命名弹层。
   *
   * @param branch 待重命名分支
   * @returns 无返回值
   */
  const openRename = (branch: GitBranchInfo) => {
    setRenameBranch(branch);
    setRenameValue(branch.name);
    props.onOpenChange(false);
  };

  /**
   * 提交本地分支重命名。
   *
   * @returns 操作完成后的 Promise
   */
  const rename = async () => {
    const newBranch = renameValue.trim();
    if (!renameBranch || !newBranch || newBranch === renameBranch.full_name) return;
    const result = await props.onOperation("rename_branch", {
      branch: renameBranch.full_name,
      new_branch: newBranch
    });
    if (result?.ok) setRenameBranch(null);
  };

  /**
   * 删除本地分支，并在未合并时询问是否强制删除。
   *
   * @param branch 待删除分支
   * @returns 操作完成后的 Promise
   */
  const remove = async (branch: GitBranchInfo) => {
    const accepted = await confirm({
      title: t("Delete branch?", "删除分支？"),
      description: branch.full_name,
      confirmLabel: t("Delete", "删除"),
      danger: true
    });
    if (!accepted) return;
    props.onOpenChange(false);
    const result = await props.onOperation("delete_branch", { branch: branch.full_name });
    if (result?.ok || !isUnmergedDelete(result)) return;
    const force = await confirm({
      title: t("Force delete branch?", "强制删除分支？"),
      description: t("The branch contains commits that have not been merged.", "此分支包含尚未合并的提交。"),
      confirmLabel: t("Force delete", "强制删除"),
      danger: true
    });
    if (force) await props.onOperation("delete_branch", { branch: branch.full_name, force: true });
  };

  /**
   * 将所选分支合并到当前分支。
   *
   * @param branch 待合并分支
   * @returns 无返回值
   */
  const merge = async (branch: GitBranchInfo) => {
    props.onOpenChange(false);
    await props.onOperation("merge_branch", { branch: branch.full_name });
  };

  /**
   * 经确认后将当前分支变基到所选分支。
   *
   * @param branch 目标分支
   * @returns 无返回值
   */
  const rebase = async (branch: GitBranchInfo) => {
    const accepted = await confirm({
      title: t("Rebase current branch?", "变基当前分支？"),
      description: t(`Rebase ${props.state.head} onto ${branch.full_name}.`, `将 ${props.state.head} 变基到 ${branch.full_name}。`),
      confirmLabel: t("Rebase", "变基")
    });
    if (!accepted) return;
    props.onOpenChange(false);
    await props.onOperation("rebase_branch", { branch: branch.full_name });
  };

  return (
    <div className="git-review-branch" ref={rootRef}>
      <Button className="git-branch-trigger" onClick={() => props.onOpenChange(!props.open)} aria-expanded={props.open}>
        <GitBranch size={14} />
        <strong title={props.state.head}>{props.state.head || "HEAD"}</strong>
        <ChevronDown size={12} className={props.open ? "open" : ""} />
      </Button>
      {(props.state.ahead || props.state.behind) ? (
        <span className="git-review-sync">
          {props.state.ahead > 0 && <b><ArrowUp size={10} />{props.state.ahead}</b>}
          {props.state.behind > 0 && <i><ArrowDown size={10} />{props.state.behind}</i>}
        </span>
      ) : null}
      {props.state.upstream && <small title={props.state.upstream}>{props.state.upstream}</small>}
      {props.open && (
        <div className="git-branch-menu">
          <div className="git-branch-create">
            <input value={createName} onChange={(event) => setCreateName(event.target.value)} placeholder={t("New branch name", "新建分支名")} spellCheck={false} />
            <Button variant="primary" disabled={!createName.trim() || props.busy} onClick={() => void create()}><Plus size={12} />{t("Create", "创建")}</Button>
          </div>
          {props.loading && <div className="git-clean">{t("Loading branches...", "正在读取分支…")}</div>}
          <BranchGroup title={t("Local branches", "本地分支")} branches={localBranches} busy={props.busy} onSelect={select} onRename={openRename} onDelete={remove} onMerge={merge} onRebase={rebase} />
          <BranchGroup title={t("Remote branches", "远程分支")} branches={remoteBranches} busy={props.busy} onSelect={select} onMerge={merge} onRebase={rebase} />
        </div>
      )}
      <Modal
        open={Boolean(renameBranch)}
        title={t("Rename branch", "重命名分支")}
        description={renameBranch?.full_name}
        size="small"
        onClose={() => setRenameBranch(null)}
        footer={(
          <>
            <Button onClick={() => setRenameBranch(null)}>{t("Cancel", "取消")}</Button>
            <Button variant="primary" onClick={() => void rename()} disabled={props.busy || !renameValue.trim()}><Pencil size={13} />{t("Rename", "重命名")}</Button>
          </>
        )}
      >
        <label className="git-branch-rename-field">
          <span>{t("New branch name", "新分支名")}</span>
          <input value={renameValue} onChange={(event) => setRenameValue(event.target.value)} spellCheck={false} />
        </label>
      </Modal>
    </div>
  );
}

/**
 * 渲染一组同类型分支。
 *
 * @param props 分组标题、分支和操作回调
 * @returns 分支分组
 */
function BranchGroup(props: {
  title: string;
  branches: GitBranchInfo[];
  busy: boolean;
  onSelect: (branch: GitBranchInfo) => void;
  onRename?: (branch: GitBranchInfo) => void;
  onDelete?: (branch: GitBranchInfo) => void;
  onMerge?: (branch: GitBranchInfo) => void;
  onRebase?: (branch: GitBranchInfo) => void;
}) {
  const { t } = useI18n();
  if (props.branches.length === 0) return null;
  return (
    <div className="git-branch-group">
      <span>{props.title}</span>
      {props.branches.map((branch) => (
        <div className={`git-branch-row${branch.current ? " active" : ""}`} key={`${branch.kind}:${branch.full_name}`}>
          <Button className="git-branch-row-main" disabled={props.busy || branch.current} onClick={() => props.onSelect(branch)}>
            {branch.current && <Check size={12} />}
            <span><strong>{branch.name}</strong>{branch.upstream && <small>{branch.upstream}</small>}</span>
          </Button>
          {(props.onRename || props.onMerge || props.onRebase || (props.onDelete && !branch.current)) && (
            <span className="git-branch-row-actions">
              {props.onRename && <Button onClick={() => props.onRename?.(branch)} disabled={props.busy} title={t("Rename branch", "重命名分支")} aria-label={t("Rename branch", "重命名分支")}><Pencil size={12} /></Button>}
              {props.onMerge && !branch.current && <Button onClick={() => void props.onMerge?.(branch)} disabled={props.busy} title={t("Merge into current branch", "合并到当前分支")} aria-label={t("Merge into current branch", "合并到当前分支")}><GitMerge size={12} /></Button>}
              {props.onRebase && !branch.current && <Button onClick={() => void props.onRebase?.(branch)} disabled={props.busy} title={t("Rebase current branch onto", "将当前分支变基到此处")} aria-label={t("Rebase current branch onto", "将当前分支变基到此处")}><GitPullRequest size={12} /></Button>}
              {props.onDelete && !branch.current && <Button onClick={() => void props.onDelete?.(branch)} disabled={props.busy} title={t("Delete branch", "删除分支")} aria-label={t("Delete branch", "删除分支")}><Trash2 size={12} /></Button>}
            </span>
          )}
        </div>
      ))}
    </div>
  );
}

/**
 * 判断删除失败是否由未合并提交导致。
 *
 * @param result Git 操作响应
 * @returns 是否应提供强制删除
 */
function isUnmergedDelete(result: GitOperationResponse | undefined): boolean {
  const message = `${result?.message ?? ""}\n${result?.stderr ?? ""}`.toLowerCase();
  return message.includes("not fully merged") || message.includes("is not an ancestor");
}
