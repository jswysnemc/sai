import { ChevronRight, FilePlus2, FolderPlus } from "../../shared/ui/icons";
import type { DragEvent, MouseEvent } from "react";
import type { FileNode } from "../../api/contracts";
import { DirectoryIcon, FileTypeIcon } from "../../shared/ui/file-icon";
import { useI18n } from "../i18n/use-i18n";
import { fileTreeGitStatusLabel, fileTreeGitStatusTone, type FileTreeGitEntry, type FileTreeGitTone } from "./use-file-tree-git";

const TREE_INDENT = 12;
const TREE_PAD = 10;

/** 渲染虚拟列表中的一行。 */
export function TreeRow({ node, depth, open, selected, multiSelected, focused, dropActive, gitEntry, directoryTone, onMouseDown, onActivate, onCreate, onTreeContextMenu, onDragStart, onDragOver, onDrop }: {
  node: FileNode;
  depth: number;
  open: boolean;
  selected: boolean;
  multiSelected?: boolean;
  focused: boolean;
  dropActive?: boolean;
  gitEntry?: FileTreeGitEntry;
  directoryTone?: FileTreeGitTone;
  onMouseDown: () => void;
  onActivate: (modifiers: { toggle: boolean; range: boolean }) => void;
  onCreate: (kind: "file" | "directory") => void;
  onTreeContextMenu: (event: MouseEvent<HTMLDivElement>) => void;
  onDragStart: (event: DragEvent<HTMLDivElement>) => void;
  onDragOver: (event: DragEvent<HTMLDivElement>) => void;
  onDrop: (event: DragEvent<HTMLDivElement>) => void;
}) {
  const { t } = useI18n();
  const directory = node.kind === "directory";
  const className = ["tree-row", selected ? "active" : "", multiSelected ? "multi-selected" : "", focused ? "focused" : "", dropActive ? "drop-target" : ""].filter(Boolean).join(" ");
  return (
    <div
      id={treeRowId(node.path)}
      role="treeitem"
      draggable
      aria-expanded={directory ? open : undefined}
      aria-selected={selected || multiSelected}
      aria-level={depth + 1}
      className={className}
      style={{ paddingLeft: TREE_PAD + depth * TREE_INDENT }}
      title={node.path}
      onMouseDown={(event) => {
        if (event.button !== 0) return;
        // Shift 点击区间选择时不要顺带选中页面文字
        if (event.shiftKey) event.preventDefault();
        onMouseDown();
      }}
      onClick={(event) => onActivate({ toggle: event.ctrlKey || event.metaKey, range: event.shiftKey })}
      onKeyDown={(event) => {
        if (event.key === "Enter") onActivate({ toggle: false, range: false });
      }}
      onContextMenu={onTreeContextMenu}
      onDragStart={onDragStart}
      onDragOver={onDragOver}
      onDrop={onDrop}
    >
      {depth > 0 && (
        <span className="tree-guides" aria-hidden>
          {Array.from({ length: depth }, (_, level) => (
            <span key={level} style={{ left: TREE_PAD + level * TREE_INDENT + 5 }} />
          ))}
        </span>
      )}
      {directory ? <ChevronRight size={12} className={open ? "tree-chevron open" : "tree-chevron"} /> : <span className="tree-chevron-spacer" />}
      {directory ? <DirectoryIcon name={node.name} expanded={open} size={14} /> : <FileTypeIcon name={node.name} size={14} />}
      <span className={gitEntry
        ? `tree-row-name git-${fileTreeGitStatusTone(gitEntry.entry)}`
        : directoryTone
          ? `tree-row-name git-${directoryTone}`
          : "tree-row-name"}>{node.name}</span>
      {directory && (
        <span className="tree-row-actions">
          <button type="button" onClick={(event) => { event.stopPropagation(); onCreate("file"); }} aria-label={t("New File", "新建文件")} title={t("New File", "新建文件")}><FilePlus2 size={14} /></button>
          <button type="button" onClick={(event) => { event.stopPropagation(); onCreate("directory"); }} aria-label={t("New Folder", "新建文件夹")} title={t("New Folder", "新建文件夹")}><FolderPlus size={14} /></button>
        </span>
      )}
      {gitEntry && <span className={`tree-row-git-status git-${fileTreeGitStatusTone(gitEntry.entry)}`}>{fileTreeGitStatusLabel(gitEntry.entry)}</span>}
    </div>
  );
}

/** 把路径收成合法的 DOM id。 */
export function treeRowId(path: string): string {
  return `file-tree-row-${encodeURIComponent(path)}`;
}

