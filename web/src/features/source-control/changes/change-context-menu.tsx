import { Copy, EyeOff, FileDiff, FileText, FolderSearch, GitCompareArrows, GitMerge, Minus, Pin, Plus, RotateCcw, Trash2 } from "lucide-react";
import { useEffect, useRef } from "react";
import type { GitStatusEntry } from "../../../api/contracts";
import type { GitOperationAction } from "../../../api/git-contracts";
import { Button } from "../../../shared/ui/button/button";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { useI18n } from "../../i18n/use-i18n";
import type { RunGitOperation } from "../types";
import type { ChangeSectionKind } from "./change-section";

type ChangeContextMenuProps = {
  x: number;
  y: number;
  repoRoot: string;
  entries: GitStatusEntry[];
  primaryPath: string;
  section: ChangeSectionKind;
  busy: boolean;
  runOperation: RunGitOperation;
  comparisonBasePath: string | null;
  onOpenChanges: (path: string, section: ChangeSectionKind) => void;
  onSelectForCompare: (path: string) => void;
  onCompareWithSelected: (path: string) => void;
  onClose: () => void;
};

/**
 * 渲染 Git 文件右键菜单并支持对当前多选集合执行操作。
 *
 * @param props 菜单位置、所选文件和操作回调
 * @returns 固定定位右键菜单
 */
export function ChangeContextMenu(props: ChangeContextMenuProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const rootRef = useRef<HTMLDivElement>(null);
  const primary = props.entries.find((entry) => entry.path === props.primaryPath) ?? props.entries[0];
  const stagePaths = props.entries
    .filter((entry) => entry.untracked || entry.conflicted || entry.worktree_status !== ".")
    .map((entry) => entry.path);
  const unstagePaths = props.entries.filter((entry) => entry.staged).map((entry) => entry.path);
  const discardPaths = props.entries
    .filter((entry) => entry.untracked || entry.worktree_status !== ".")
    .map((entry) => entry.path);

  useEffect(() => {
    /** 点击菜单外部或按 Escape 时关闭文件菜单。 */
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

  /** 执行批量 Git 操作并关闭菜单。 */
  const run = async (action: GitOperationAction, targetPaths: string[]) => {
    props.onClose();
    await props.runOperation(action, { paths: targetPaths });
  };

  /** 经确认后永久丢弃所选工作区修改。 */
  const discard = async () => {
    const accepted = await confirm({
      title: t(`Discard ${discardPaths.length} selected changes?`, `丢弃选中的 ${discardPaths.length} 项改动？`),
      description: t("Tracked files will be restored and untracked files will be permanently deleted.", "已跟踪文件将恢复，未跟踪文件将永久删除。"),
      confirmLabel: t("Discard", "丢弃"),
      danger: true
    });
    if (accepted) await run("discard", discardPaths);
  };

  /** 打开工作区文件，并可要求展开文件树。 */
  const openFile = (reveal: boolean) => {
    if (!primary) return;
    const path = absoluteRepositoryPath(props.repoRoot, primary.path);
    window.dispatchEvent(new CustomEvent("sai:open-file", { detail: { path, reveal } }));
    props.onClose();
  };

  /** 复制绝对或仓库相对路径。 */
  const copyPath = async (relative: boolean) => {
    if (!primary) return;
    const value = relative ? primary.path : absoluteRepositoryPath(props.repoRoot, primary.path);
    try {
      await navigator.clipboard.writeText(value);
    } finally {
      props.onClose();
    }
  };

  if (!primary) return null;
  const untrackedPaths = props.entries.filter((entry) => entry.untracked).map((entry) => entry.path);
  return (
    <div ref={rootRef} className="git-change-context-menu" role="menu" style={{ left: props.x, top: props.y }}>
      <Button onClick={() => openFile(false)}><FileText size={12} />{t("Open File", "打开文件")}</Button>
      <Button onClick={() => { props.onOpenChanges(primary.path, props.section); props.onClose(); }}><FileDiff size={12} />{t("Open Changes", "打开变更")}</Button>
      {primary.conflicted && <Button onClick={() => { props.onOpenChanges(primary.path, "merge"); props.onClose(); }}><GitMerge size={12} />{t("Open in Merge Editor", "在合并编辑器中打开")}</Button>}
      <Button onClick={() => { props.onSelectForCompare(primary.path); props.onClose(); }}><Pin size={12} />{t("Select for Compare", "选择以进行比较")}</Button>
      {props.comparisonBasePath && props.comparisonBasePath !== primary.path && (
        <Button
          title={t(`Compare ${primary.path} with ${props.comparisonBasePath}`, `将 ${primary.path} 与 ${props.comparisonBasePath} 比较`)}
          onClick={() => { props.onCompareWithSelected(primary.path); props.onClose(); }}
        >
          <GitCompareArrows size={12} />{t("Compare with Selected", "与所选文件比较")}
        </Button>
      )}
      <span>{t("Changes", "改动")}</span>
      {stagePaths.length > 0 && <Button disabled={props.busy} onClick={() => void run("stage", stagePaths)}><Plus size={12} />{t(`Stage ${stagePaths.length} Selected`, `暂存选中的 ${stagePaths.length} 项`)}</Button>}
      {unstagePaths.length > 0 && <Button disabled={props.busy} onClick={() => void run("unstage", unstagePaths)}><Minus size={12} />{t(`Unstage ${unstagePaths.length} Selected`, `取消暂存选中的 ${unstagePaths.length} 项`)}</Button>}
      {untrackedPaths.length > 0 && <Button disabled={props.busy} onClick={() => void run("add_to_gitignore", untrackedPaths)}><EyeOff size={12} />{t("Add to .gitignore", "加入 .gitignore")}</Button>}
      {discardPaths.length > 0 && <Button className="danger" disabled={props.busy} onClick={() => void discard()}>{props.entries.some((entry) => entry.untracked) ? <Trash2 size={12} /> : <RotateCcw size={12} />}{t("Discard Selected", "丢弃所选改动")}</Button>}
      <span>{t("Path", "路径")}</span>
      <Button onClick={() => openFile(true)}><FolderSearch size={12} />{t("Reveal in Explorer", "在资源管理器中显示")}</Button>
      <Button onClick={() => void copyPath(false)}><Copy size={12} />{t("Copy Path", "复制路径")}</Button>
      <Button onClick={() => void copyPath(true)}><Copy size={12} />{t("Copy Relative Path", "复制相对路径")}</Button>
    </div>
  );
}

/**
 * 组合仓库根目录和 Git 相对路径，保留跨平台分隔符兼容性。
 *
 * @param repoRoot 仓库绝对路径
 * @param relativePath Git 相对路径
 * @returns 文件绝对路径
 */
function absoluteRepositoryPath(repoRoot: string, relativePath: string): string {
  return `${repoRoot.replace(/[\\/]$/, "")}/${relativePath.replace(/^[\\/]/, "")}`;
}
