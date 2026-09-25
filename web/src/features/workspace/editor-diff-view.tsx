import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { createPortal } from "react-dom";
import { api } from "../../api/client";
import type { GitOperationAction } from "../../api/git-contracts";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { parseDiff } from "../chat/tool-renderers/diff/diff-parser";
import type { DiffLine } from "../chat/tool-renderers/diff/diff-model";
import { EditorDiffBody } from "./editor-diff-body";
import { useI18n } from "../i18n/use-i18n";
import { splitGitPatchHunks } from "../source-control/diff/partial-diff";
import { refreshKeysFor } from "../source-control/state/git-refresh-keys";
import { useClampedMenuPosition } from "./menu-position";

type EditorDiffViewProps = {
  path: string;
  /** 仓库内路径，Git 操作使用它而不是工作区相对路径 */
  gitPath: string;
  patch: string;
  loading: boolean;
  repoRoot: string;
  untracked: boolean;
  /** 已暂存且工作区没有额外改动时，右键提供取消暂存 */
  stagedClean: boolean;
  /** 工作区相对暂存区仍有改动时，右键提供暂存和丢弃 */
  worktreeDirty: boolean;
};

type DiffMenu = { x: number; y: number; index: number; lineText?: string };

/**
 * 在编辑区展示当前文件相对 Git 的统一差异，并支持对所点区块做右键操作。
 *
 * @param props 路径、补丁、仓库和加载状态
 * @returns 差异正文或空状态
 */
export function EditorDiffView({
  path,
  gitPath,
  patch,
  loading,
  repoRoot,
  untracked,
  stagedClean,
  worktreeDirty
}: EditorDiffViewProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const file = useMemo(() => (patch ? parseDiff(patch)[0] ?? null : null), [patch]);
  const hunks = useMemo(() => splitGitPatchHunks(patch), [patch]);
  const [menu, setMenu] = useState<DiffMenu | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const language = path.split(".").at(-1);
  const clicked = file && menu && menu.index >= 0 ? file.lines[menu.index] : undefined;
  const lineText = menu?.lineText ?? clicked?.text;
  const hunkIndex = file && menu ? hunkIndexAt(file.lines, menu.index, hunks.length) : -1;
  const hunk = hunkIndex >= 0 ? hunks[hunkIndex] : undefined;
  const canPatch = Boolean(hunk && repoRoot && !untracked);

  /**
   * 记下右键所在的差异行。
   *
   * @param event 差异区域的右键事件
   */
  const openMenu = (event: MouseEvent<HTMLDivElement>) => {
    event.preventDefault();
    const row = (event.target as HTMLElement).closest<HTMLElement>("[data-diff-row]");
    const raw = row?.dataset.diffIndex;
    const index = raw === undefined ? -1 : Number(raw);
    setMenu({
      x: event.clientX,
      y: event.clientY,
      index: Number.isFinite(index) ? index : -1,
      lineText: row?.dataset.lineText
    });
  };

  /**
   * 把文本写入剪贴板并关掉菜单。
   *
   * @param value 要复制的文本
   */
  const copy = (value: string) => {
    setMenu(null);
    void navigator.clipboard.writeText(value);
  };

  /**
   * 执行补丁或整文件 Git 操作，并刷新差异。
   *
   * @param action Git 操作
   * @param body 操作参数
   */
  const runGit = async (action: GitOperationAction, body: { patch?: string; paths?: string[] }) => {
    if (!repoRoot) return;
    if (action === "discard_patch") {
      const accepted = await confirm({
        title: t("Discard this change?", "丢弃此区块？"),
        description: t("This restores the selected hunk. The change cannot be undone.", "将还原这个区块，且无法撤销。"),
        confirmLabel: t("Discard", "丢弃"),
        danger: true
      });
      if (!accepted) return;
    }
    setMenu(null);
    setBusy(true);
    setError(null);
    try {
      const result = await api.workspace.gitOp(action, { repo_root: repoRoot, path: gitPath, ...body });
      if (!result.ok) {
        setError(result.message || result.stderr || t("Git operation failed", "Git 操作失败"));
        return;
      }
      await Promise.all([
        ...refreshKeysFor(action).map((key) => queryClient.invalidateQueries({ queryKey: [key] })),
        queryClient.invalidateQueries({ queryKey: ["file-tree-git-statuses"] }),
        queryClient.invalidateQueries({ queryKey: ["file", path] })
      ]);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : t("Git operation failed", "Git 操作失败"));
    } finally {
      setBusy(false);
    }
  };

  if (loading) return <div className="editor-state">{t("Loading diff", "正在读取差异")}</div>;
  if (!file || file.lines.length === 0) {
    return <div className="editor-state">{t("No changes to review", "没有可查看的差异")}</div>;
  }

  return (
    <div className="editor-diff" onContextMenu={openMenu}>
      {error && <p className="pane-error">{error}</p>}
      <EditorDiffBody file={{ ...file, path }} path={path} language={language} />
      {menu && (
        <DiffContextMenu
          x={menu.x}
          y={menu.y}
          lineText={lineText}
          hunkText={hunk?.patch}
          canStage={canPatch && worktreeDirty}
          canDiscard={canPatch && worktreeDirty}
          canUnstage={canPatch && stagedClean}
          canStageFile={Boolean(repoRoot && untracked)}
          busy={busy}
          path={path}
          onCopy={copy}
          onStage={() => void runGit("stage_patch", { patch: hunk?.patch })}
          onUnstage={() => void runGit("unstage_patch", { patch: hunk?.patch })}
          onDiscard={() => void runGit("discard_patch", { patch: hunk?.patch })}
          onStageFile={() => void runGit("stage", { paths: [gitPath] })}
          onClose={() => setMenu(null)}
        />
      )}
    </div>
  );
}

/**
 * 找到差异行所属的 hunk 序号。
 *
 * 解析器会省略没有折叠区间的首个 hunk 头，因此正文开头默认属于第 0 个 hunk；
 * 只有第一行本身就是 hunk 头时，才从 -1 开始计数。
 *
 * @param lines 解析后的差异行
 * @param index 右键行下标；不在行上时为 -1
 * @param hunkCount 原始补丁拆出的 hunk 数量
 * @returns hunk 序号；点在区块之外时为 -1
 */
export function hunkIndexAt(lines: DiffLine[], index: number, hunkCount: number): number {
  if (index < 0 || hunkCount === 0) return -1;
  let hunk = lines[0]?.kind === "hunk" ? -1 : 0;
  for (let cursor = 0; cursor <= index && cursor < lines.length; cursor += 1) {
    if (lines[cursor].kind === "hunk") hunk += 1;
  }
  if (hunk < 0 || hunk >= hunkCount) return -1;
  return hunk;
}

type DiffContextMenuProps = {
  x: number;
  y: number;
  lineText?: string;
  hunkText?: string;
  canStage: boolean;
  canDiscard: boolean;
  canUnstage: boolean;
  canStageFile: boolean;
  busy: boolean;
  path: string;
  onCopy: (value: string) => void;
  onStage: () => void;
  onUnstage: () => void;
  onDiscard: () => void;
  onStageFile: () => void;
  onClose: () => void;
};

/**
 * 渲染差异视图右键菜单。
 *
 * @param props 坐标、所点行和操作回调
 * @returns 固定定位菜单
 */
function DiffContextMenu(props: DiffContextMenuProps) {
  const { t } = useI18n();
  const ref = useRef<HTMLDivElement>(null);
  const position = useClampedMenuPosition(props.x, props.y, ref);
  const gitActions = props.canStage || props.canDiscard || props.canUnstage || props.canStageFile;
  useEffect(() => {
    const close = (event: PointerEvent) => { if (!ref.current?.contains(event.target as Node)) props.onClose(); };
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") props.onClose(); };
    document.addEventListener("pointerdown", close);
    document.addEventListener("keydown", escape);
    return () => { document.removeEventListener("pointerdown", close); document.removeEventListener("keydown", escape); };
  }, [props]);

  return createPortal(
    <div ref={ref} className="editor-diff-menu" role="menu" style={position}>
      {props.lineText !== undefined && (
        <button type="button" role="menuitem" onClick={() => props.onCopy(props.lineText ?? "")}>{t("Copy line", "复制此行")}</button>
      )}
      {props.hunkText && (
        <button type="button" role="menuitem" onClick={() => props.onCopy(props.hunkText ?? "")}>{t("Copy hunk", "复制此区块")}</button>
      )}
      {(props.lineText !== undefined || props.hunkText) && <div className="file-tree-context-separator" role="separator" />}
      {props.canStage && (
        <button type="button" role="menuitem" disabled={props.busy} onClick={props.onStage}>{t("Stage hunk", "暂存此区块")}</button>
      )}
      {props.canUnstage && (
        <button type="button" role="menuitem" disabled={props.busy} onClick={props.onUnstage}>{t("Unstage hunk", "取消暂存此区块")}</button>
      )}
      {props.canDiscard && (
        <button type="button" role="menuitem" className="danger" disabled={props.busy} onClick={props.onDiscard}>{t("Discard hunk", "丢弃此区块")}</button>
      )}
      {props.canStageFile && (
        <button type="button" role="menuitem" disabled={props.busy} onClick={props.onStageFile}>{t("Stage file", "暂存此文件")}</button>
      )}
      {gitActions && <div className="file-tree-context-separator" role="separator" />}
      <button type="button" role="menuitem" onClick={() => props.onCopy(props.path)}>{t("Copy Relative Path", "复制相对路径")}</button>
    </div>,
    document.body
  );
}
