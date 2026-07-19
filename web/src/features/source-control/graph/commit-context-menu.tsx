import { CheckCircle2, Cherry, Copy, GitBranch, LogOut, RotateCcw } from "lucide-react";
import { useEffect, useRef } from "react";
import type { GitCommitSummary } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import type { GitOperationUiOptions, RunGitOperation } from "../types";

type CommitContextMenuProps = {
  commit: GitCommitSummary;
  x: number;
  y: number;
  busy: boolean;
  runOperation: RunGitOperation;
  onView: () => void;
  onCreateBranch: () => void;
  onClose: () => void;
};

/**
 * 渲染提交图右键菜单，并映射到真实 Git 历史命令。
 *
 * @param props 提交、屏幕位置和操作回调
 * @returns 固定定位的提交菜单
 */
export function CommitContextMenu(props: CommitContextMenuProps) {
  const { t } = useI18n();
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    /** 点击菜单外部或按 Escape 时关闭菜单。 */
    const closeOutside = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) props.onClose();
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") props.onClose();
    };
    document.addEventListener("pointerdown", closeOutside);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeOutside);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [props]);

  /**
   * 执行提交操作并关闭菜单。
   *
   * @param action Git 操作名称
   * @param options 操作参数和可选确认信息
   * @returns 无返回值
   */
  const run = async (action: string, options: GitOperationUiOptions = {}) => {
    props.onClose();
    await props.runOperation(action, { commit: props.commit.sha, ...options });
  };

  /**
   * 复制提交哈希或说明。
   *
   * @param value 待复制文本
   * @returns 无返回值
   */
  const copy = async (value: string) => {
    props.onClose();
    try {
      await navigator.clipboard.writeText(value);
    } catch {
      return;
    }
  };

  return (
    <div ref={rootRef} className="git-commit-context-menu" role="menu" style={{ left: props.x, top: props.y }}>
      <Button onClick={() => { props.onView(); props.onClose(); }}><CheckCircle2 size={12} />{t("View Changes", "查看变更")}</Button>
      <Button disabled={props.busy} onClick={() => void run("checkout_commit")}><LogOut size={12} />{t("Checkout (Detached)", "检出（分离状态）")}</Button>
      <Button disabled={props.busy} onClick={() => { props.onCreateBranch(); props.onClose(); }}><GitBranch size={12} />{t("Create Branch From...", "从此处创建分支…")}</Button>
      <span>{t("History", "历史")}</span>
      <Button disabled={props.busy} onClick={() => void run("cherry_pick")}><Cherry size={12} />{t("Cherry-Pick", "拣选")}</Button>
      <Button disabled={props.busy} onClick={() => void run("rebase_onto", {
        confirmTitle: t("Rebase current branch?", "变基当前分支？"),
        confirmDescription: t(`Rebase the current branch onto ${props.commit.short_sha}.`, `将当前分支变基到 ${props.commit.short_sha}。`)
      })}><GitBranch size={12} />{t("Rebase Current Branch Onto", "将当前分支变基到此处")}</Button>
      <Button disabled={props.busy} onClick={() => void run("revert_commit")}><RotateCcw size={12} />{t("Revert Commit", "还原提交")}</Button>
      <span>{t("Reset Current Branch", "重置当前分支")}</span>
      <Button disabled={props.busy} onClick={() => void run("reset_commit", { reset_mode: "soft" })}>{t("Reset Soft", "软重置")}</Button>
      <Button disabled={props.busy} onClick={() => void run("reset_commit", { reset_mode: "mixed" })}>{t("Reset Mixed", "混合重置")}</Button>
      <Button disabled={props.busy} onClick={() => void run("reset_commit", {
        reset_mode: "hard",
        confirmTitle: t("Hard reset current branch?", "硬重置当前分支？"),
        confirmDescription: t("All staged and working tree changes will be permanently discarded.", "全部已暂存和工作树修改将永久丢失。")
      })}>{t("Reset Hard", "硬重置")}</Button>
      <span>{t("Copy", "复制")}</span>
      <Button onClick={() => void copy(props.commit.sha)}><Copy size={12} />{t("Copy Commit ID", "复制提交 ID")}</Button>
      <Button onClick={() => void copy(props.commit.subject)}><Copy size={12} />{t("Copy Commit Message", "复制提交说明")}</Button>
    </div>
  );
}
