import { Check, ChevronDown } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { GitOperationOptions } from "../../../api/git-contracts";
import { Button } from "../../../shared/ui/button/button";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { TextArea } from "../../../shared/ui/form/text-area";
import { useI18n } from "../../i18n/use-i18n";
import { applyCommitConfig, resolveMainCommitKind } from "./commit-behavior";

type CommitControlProps = {
  message: string;
  stagedCount: number;
  workingCount: number;
  conflictedCount: number;
  busy: boolean;
  enableSmartCommit: boolean;
  suggestSmartCommit: boolean;
  showActionButton: boolean;
  confirmEmptyCommits: boolean;
  confirmSync: boolean;
  postCommitCommand: "none" | "push" | "sync";
  untrackedChanges: "mixed" | "separate" | "hidden";
  onMessageChange: (message: string) => void;
  onCommit: (options: GitOperationOptions) => Promise<boolean>;
};

type CommitChoice = {
  key: string;
  label: string;
  options: GitOperationOptions;
  requiresStaged?: boolean;
  requiresWorking?: boolean;
  requiresClean?: boolean;
  promptsForSmartCommit?: boolean;
};

/**
 * 渲染提交说明、主提交动作和提交变体菜单。
 *
 * @param props 提交状态、说明和操作回调
 * @returns VS Code 风格提交输入区
 */
export function CommitControl(props: CommitControlProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const rootRef = useRef<HTMLDivElement>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const hasMessage = Boolean(props.message.trim());
  const hasConflicts = props.conflictedCount > 0;
  const mainKind = resolveMainCommitKind(
    props.stagedCount,
    props.workingCount,
    props.enableSmartCommit,
    props.suggestSmartCommit
  );
  const mainChoice: CommitChoice = mainKind === "staged"
    ? { key: "staged", label: t("Commit Staged", "提交已暂存"), options: {}, requiresStaged: true }
    : mainKind === "all"
      ? { key: "all", label: t("Commit All", "提交全部"), options: { all: true }, requiresWorking: true }
      : mainKind === "suggest_all"
        ? { key: "smart", label: t("Commit", "提交"), options: { all: true }, requiresWorking: true, promptsForSmartCommit: true }
        : { key: "disabled", label: t("Commit", "提交"), options: {}, requiresStaged: true };
  const choices: CommitChoice[] = [
    mainChoice,
    { key: "staged-signoff", label: t("Commit Staged (Signed Off)", "提交已暂存并签署"), options: { signoff: true }, requiresStaged: true },
    { key: "all-signoff", label: t("Commit All (Signed Off)", "提交全部并签署"), options: { all: true, signoff: true }, requiresWorking: true },
    { key: "staged-push", label: t("Commit Staged & Push", "提交已暂存并推送"), options: { post_action: "push" }, requiresStaged: true },
    { key: "all-push", label: t("Commit All & Push", "提交全部并推送"), options: { all: true, post_action: "push" }, requiresWorking: true },
    { key: "staged-sync", label: t("Commit Staged & Sync", "提交已暂存并同步"), options: { post_action: "sync" }, requiresStaged: true },
    { key: "all-sync", label: t("Commit All & Sync", "提交全部并同步"), options: { all: true, post_action: "sync" }, requiresWorking: true },
    { key: "amend-message", label: t("Amend Last Commit", "修订上次提交"), options: { amend: true } },
    { key: "amend-staged", label: t("Amend with Staged", "使用已暂存修订"), options: { amend: true }, requiresStaged: true },
    { key: "amend-all", label: t("Amend with All", "使用全部修改修订"), options: { all: true, amend: true }, requiresWorking: true },
    { key: "empty", label: t("Commit Empty", "创建空提交"), options: { allow_empty: true }, requiresClean: true }
  ];

  useEffect(() => {
    if (!menuOpen) return;
    /** 点击控件外部时关闭提交菜单。 */
    const closeOutside = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setMenuOpen(false);
    };
    document.addEventListener("pointerdown", closeOutside);
    return () => document.removeEventListener("pointerdown", closeOutside);
  }, [menuOpen]);

  /**
   * 执行指定提交变体，并在成功后关闭菜单。
   *
   * @param choice 提交动作定义
   * @returns 无返回值
   */
  const commit = async (choice: CommitChoice) => {
    if (!canRunChoice(choice, props, hasMessage)) return;
    // 1. Smart Commit 未直接启用时，使用统一确认框提示提交全部可见文件
    if (choice.promptsForSmartCommit) {
      const accepted = await confirm({
        title: t("Commit all changes?", "提交全部改动？"),
        description: t("There are no staged changes. Commit all visible working tree changes instead?", "当前没有已暂存改动，是否改为提交全部可见工作区改动？"),
        confirmLabel: t("Commit All", "提交全部")
      });
      if (!accepted) return;
    }

    const options = applyCommitConfig(choice.options, {
      post_commit_command: props.postCommitCommand,
      untracked_changes: props.untrackedChanges
    });
    // 2. 空提交和同步按配置保留明确确认
    if (options.allow_empty && props.confirmEmptyCommits) {
      const accepted = await confirm({
        title: t("Create empty commit?", "创建空提交？"),
        description: t("This commit records a message without changing repository files.", "该提交只记录提交说明，不包含文件改动。"),
        confirmLabel: t("Create Commit", "创建提交")
      });
      if (!accepted) return;
    }
    if (options.post_action === "sync" && props.confirmSync) {
      const accepted = await confirm({
        title: t("Commit and sync?", "提交并同步？"),
        description: t("After committing, Git will pull remote changes and push the current branch.", "提交后 Git 将获取远端改动并推送当前分支。"),
        confirmLabel: t("Commit and Sync", "提交并同步")
      });
      if (!accepted) return;
    }

    const succeeded = await props.onCommit(options);
    if (succeeded) setMenuOpen(false);
  };

  /**
   * 处理提交输入区快捷键。
   *
   * @param event 文本域键盘事件
   * @returns 无返回值
   */
  const handleKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== "Enter" || (!event.ctrlKey && !event.metaKey)) return;
    event.preventDefault();
    void commit(mainChoice);
  };

  return (
    <div className="git-commit-box" ref={rootRef}>
      <TextArea
        rows={3}
        value={props.message}
        onChange={(event) => props.onMessageChange(event.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={t("Message (Ctrl+Enter to commit)", "提交说明（Ctrl+Enter 提交）")}
      />
      {props.showActionButton && <div className="git-commit-actions">
        <Button
          variant="primary"
          className="git-commit-primary"
          onClick={() => void commit(mainChoice)}
          disabled={!canRunChoice(mainChoice, props, hasMessage)}
        >
          <Check size={13} />
          {mainChoice.label}
        </Button>
        <Button
          variant="primary"
          className="git-commit-menu-trigger"
          onClick={() => setMenuOpen((value) => !value)}
          disabled={props.busy || hasConflicts}
          aria-expanded={menuOpen}
          aria-label={t("Choose commit action", "选择提交操作")}
        >
          <ChevronDown size={13} />
        </Button>
        {menuOpen && (
          <div className="git-commit-menu" role="menu">
            {choices.map((choice) => (
              <Button
                key={choice.key}
                className="git-commit-menu-item"
                disabled={!canRunChoice(choice, props, hasMessage)}
                onClick={() => void commit(choice)}
              >
                {choice.label}
              </Button>
            ))}
          </div>
        )}
      </div>}
      {hasConflicts && <small className="git-commit-blocked">{t("Resolve all conflicts before committing.", "解决全部冲突后才能提交。")}</small>}
    </div>
  );
}

/**
 * 判断提交变体是否满足当前仓库条件。
 *
 * @param choice 提交动作定义
 * @param props 当前提交区状态
 * @param hasMessage 是否存在非空说明
 * @returns 是否允许执行
 */
function canRunChoice(
  choice: CommitChoice,
  props: Pick<CommitControlProps, "stagedCount" | "workingCount" | "conflictedCount" | "busy">,
  hasMessage: boolean
): boolean {
  if (props.busy || props.conflictedCount > 0 || !hasMessage) return false;
  if (choice.requiresStaged && props.stagedCount === 0) return false;
  if (choice.requiresWorking && props.workingCount === 0) return false;
  if (choice.requiresClean && (props.stagedCount > 0 || props.workingCount > 0)) return false;
  return true;
}
