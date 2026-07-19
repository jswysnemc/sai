import { GitBranchPlus, Plus } from "lucide-react";
import { useState } from "react";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import type { RunGitOperation } from "../types";

type WorktreeControlsProps = {
  busy: boolean;
  repositoryRoot: string | null;
  runOperation: RunGitOperation;
};

/**
 * 渲染新 worktree 路径、分支和起点输入控件。
 *
 * @param props 选中仓库、忙碌状态和 Git 操作回调
 * @returns worktree 创建控件
 */
export function WorktreeControls(props: WorktreeControlsProps) {
  const { t } = useI18n();
  const [path, setPath] = useState("");
  const [newBranch, setNewBranch] = useState("");
  const [startPoint, setStartPoint] = useState("");

  /**
   * 创建 worktree，并在成功后清空输入。
   *
   * @returns 无返回值
   */
  const create = async () => {
    if (!props.repositoryRoot || !path.trim()) return;
    const result = await props.runOperation("worktree_add", {
      worktree_path: path.trim(),
      branch: startPoint.trim() || undefined,
      new_branch: newBranch.trim() || undefined
    });
    if (!result?.ok) return;
    setPath("");
    setNewBranch("");
    setStartPoint("");
  };

  return (
    <div className="git-worktree-create">
      <span><GitBranchPlus size={13} />{t("Create worktree", "创建 worktree")}</span>
      <input value={path} onChange={(event) => setPath(event.target.value)} placeholder={t("Path", "路径")} spellCheck={false} />
      <input value={newBranch} onChange={(event) => setNewBranch(event.target.value)} placeholder={t("New branch (optional)", "新分支（可选）")} spellCheck={false} />
      <input value={startPoint} onChange={(event) => setStartPoint(event.target.value)} placeholder={t("Start point (optional)", "起点（可选）")} spellCheck={false} />
      <Button
        variant="primary"
        disabled={props.busy || !props.repositoryRoot || !path.trim()}
        onClick={() => void create()}
      >
        <Plus size={12} />{t("Create", "创建")}
      </Button>
    </div>
  );
}
