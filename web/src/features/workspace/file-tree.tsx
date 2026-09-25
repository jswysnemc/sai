import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronRight, ChevronsDownUp, FilePlus2, FolderPlus, PanelRightClose, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent, type UIEvent } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import type { FileNode } from "../../api/contracts";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { DirectoryIcon, FileTypeIcon } from "../../shared/ui/file-icon";
import { readExpandedDirectories, writeExpandedDirectories } from "./file-tree-expansion";
import {
  applyLazyChildren,
  directoryPaths,
  filterFileNodes,
  findFileNode,
  flattenVisibleNodes,
  parentFilePath,
  truncatedDirectoryFor,
  visibleRowRange,
  withAncestors
} from "./file-tree-utils";
import { WorkspaceFileSearch } from "./workspace-file-search";
import { useI18n } from "../i18n/use-i18n";
import { Button } from "../../shared/ui/button/button";
import { TextInput } from "../../shared/ui/form/text-input";
import { absoluteWorkspacePath, pasteTargetPath, uniqueCopyPath, type TreeClipboard } from "./file-tree-clipboard";
import { REVEAL_FILE_TREE_PATH_EVENT } from "./files-home";
import { FileTreeContextMenu } from "./file-tree-context-menu";
import {
  directoryGitTones,
  fileTreeGitStatusLabel,
  fileTreeGitStatusTone,
  useFileTreeGit,
  type FileTreeGitEntry,
  type FileTreeGitTone
} from "./use-file-tree-git";

const TREE_INDENT = 12;
const TREE_PAD = 10;

type FileTreeProps = {
  selectedFile: string | null;
  onSelectFile: (path: string) => void;
  onClearFile: () => void;
  onClose?: () => void;
  showHeading?: boolean;
  workspaceLabel?: string;
  workspaceKey?: string;
  searchPlaceholder?: string;
};

type FileAction = { kind: "file" | "directory" | "rename"; value: string } | null;
type TreeMenuState = { x: number; y: number; path: string; directory: boolean } | null;

/**
 * 渲染支持创建、重命名和删除的工作区文件树。
 *
 * 可见行压平后按窗口挂载，展开状态按工作区记住。深度被接口截断的目录在展开时再读一层。
 *
 * @param props 当前文件选择、更新回调与关闭文件树回调
 * @returns 文件浏览器
 */
export function FileTree({ selectedFile, onSelectFile, onClearFile, onClose, showHeading = true, workspaceLabel, workspaceKey, searchPlaceholder }: FileTreeProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const git = useFileTreeGit();
  const listed = useQuery({
    queryKey: ["workspaces"],
    queryFn: api.workspaces.list,
    staleTime: 60_000
  });
  const storageKey = workspaceKey ?? listed.data?.active_id ?? "active";
  const tree = useQuery({ queryKey: ["file-tree"], queryFn: () => api.workspace.tree(), refetchOnWindowFocus: true, refetchInterval: 15_000 });
  const [storageKeySeen, setStorageKeySeen] = useState(storageKey);
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(() => withAncestors(readExpandedDirectories(storageKey), selectedFile));
  const [lazyChildren, setLazyChildren] = useState<ReadonlyMap<string, FileNode[]>>(() => new Map());
  const [focusedPath, setFocusedPath] = useState<string | null>(selectedFile);
  const [action, setAction] = useState<FileAction>(null);
  const [search, setSearch] = useState("");
  const [error, setError] = useState<Error | null>(null);
  const [treeMenu, setTreeMenu] = useState<TreeMenuState>(null);
  const [clipboard, setClipboard] = useState<TreeClipboard | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewport, setViewport] = useState(0);
  const [rowHeight, setRowHeight] = useState(28);
  const scrollRef = useRef<HTMLDivElement>(null);
  const probeRef = useRef<HTMLDivElement>(null);
  const scrollFrame = useRef(0);
  const inFlight = useRef(new Set<string>());
  const lazyPathsRef = useRef(lazyChildren);
  const expandedRef = useRef(expanded);
  const pinnedPath = useRef<string | null>(null);
  lazyPathsRef.current = lazyChildren;
  expandedRef.current = expanded;

  if (storageKeySeen !== storageKey) {
    setStorageKeySeen(storageKey);
    setExpanded(withAncestors(readExpandedDirectories(storageKey), selectedFile));
    setLazyChildren(new Map());
    pinnedPath.current = null;
  }

  const source = useMemo(() => applyLazyChildren(tree.data ?? [], lazyChildren), [tree.data, lazyChildren]);
  const workspaceRoot = listed.data?.workspaces.find((item) => item.id === listed.data?.active_id)?.path ?? "";
  const searching = Boolean(search.trim());
  const rows = useMemo(() => {
    const filtered = filterFileNodes(source, search);
    const open = searching ? directoryPaths(filtered) : expanded;
    return flattenVisibleNodes(filtered, open);
  }, [source, search, searching, expanded]);
  const directoryTones = useMemo(() => directoryGitTones(git.entries), [git.entries]);
  const focusedIndex = focusedPath ? rows.findIndex((row) => row.node.path === focusedPath) : -1;
  const range = visibleRowRange(rows.length, scrollTop, viewport, rowHeight);
  const windowRows = rows.slice(range.start, range.end);

  const loadDirectory = useCallback(async (path: string) => {
    if (inFlight.current.has(path)) return;
    inFlight.current.add(path);
    try {
      const children = await api.workspace.tree(path, 2);
      setLazyChildren((current) => {
        const next = new Map(current);
        next.set(path, children);
        return next;
      });
      setError(null);
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to read directory", "读取目录失败"));
    } finally {
      inFlight.current.delete(path);
    }
  }, []);

  useEffect(() => {
    writeExpandedDirectories(storageKey, expanded);
  }, [storageKey, expanded]);

  useEffect(() => {
    if (!selectedFile) return;
    setFocusedPath(selectedFile);
    setExpanded((current) => withAncestors(current, selectedFile));
  }, [selectedFile]);

  useEffect(() => {
    const reveal = (event: Event) => {
      const path = (event as CustomEvent<string>).detail;
      if (!path) return;
      setFocusedPath(path);
      setExpanded((current) => withAncestors(current, path));
    };
    window.addEventListener(REVEAL_FILE_TREE_PATH_EVENT, reveal);
    return () => window.removeEventListener(REVEAL_FILE_TREE_PATH_EVENT, reveal);
  }, []);

  useEffect(() => {
    if (!selectedFile) return;
    const truncated = truncatedDirectoryFor(source, selectedFile);
    if (truncated && !lazyChildren.has(truncated)) void loadDirectory(truncated);
  }, [selectedFile, source, lazyChildren, loadDirectory]);

  useEffect(() => {
    if (!tree.dataUpdatedAt) return;
    for (const path of lazyPathsRef.current.keys()) {
      if (expandedRef.current.has(path)) void loadDirectory(path);
    }
  }, [tree.dataUpdatedAt, loadDirectory]);

  useLayoutEffect(() => {
    const rowsEl = scrollRef.current;
    if (!rowsEl) return;
    const measure = () => {
      setViewport(rowsEl.clientHeight);
      const height = probeRef.current?.offsetHeight ?? 0;
      if (height > 0) setRowHeight(height);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(rowsEl);
    return () => observer.disconnect();
  }, []);

  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el || !focusedPath || pinnedPath.current === focusedPath) return;
    const index = rows.findIndex((row) => row.node.path === focusedPath);
    if (index < 0) return;
    pinnedPath.current = focusedPath;
    const top = index * rowHeight;
    const bottom = top + rowHeight;
    if (top < el.scrollTop) {
      el.scrollTop = top;
      setScrollTop(top);
    } else if (bottom > el.scrollTop + el.clientHeight) {
      const next = Math.max(0, bottom - el.clientHeight);
      el.scrollTop = next;
      setScrollTop(next);
    }
  }, [focusedPath, rows, rowHeight]);

  useEffect(() => () => {
    if (scrollFrame.current) cancelAnimationFrame(scrollFrame.current);
  }, []);

  /** 展开或收起目录；子节点还没加载时补读。 */
  const toggleDirectory = (node: FileNode) => {
    const opening = !expanded.has(node.path);
    setExpanded((current) => {
      const next = new Set(current);
      if (opening) next.add(node.path);
      else next.delete(node.path);
      return next;
    });
    if (opening && node.children.length === 0 && !lazyChildren.has(node.path)) void loadDirectory(node.path);
  };

  /** 重新拉取文件树，并补回当前展开的深层目录。 */
  const reloadTree = async () => {
    const reopen = [...lazyPathsRef.current.keys()].filter((path) => expandedRef.current.has(path));
    setLazyChildren(new Map());
    await refreshWorkspaceQueries(queryClient);
    await Promise.all(reopen.map((path) => loadDirectory(path)));
  };

  /** 收起全部目录。 */
  const collapseAll = () => setExpanded(new Set());

  /** 展开父目录，并在该目录下打开新建输入栏。 */
  const beginCreate = (kind: "file" | "directory", targetPath = focusedPath) => {
    const target = findFileNode(source, targetPath);
    const parent = target?.kind === "directory" ? target.path : parentFilePath(target?.path ?? "");
    if (parent) setExpanded((current) => new Set(current).add(parent));
    setAction({ kind, value: parent ? `${parent}/` : "" });
    setError(null);
  };

  /** 展开并聚焦条目所在的目录。 */
  const revealContaining = (path: string) => {
    const parent = parentFilePath(path);
    setExpanded((current) => withAncestors(current, path));
    setFocusedPath(parent || path);
  };

  /** 把剪贴板里的文件移动或复制到目录。 */
  const pasteInto = async (directory: string) => {
    if (!clipboard) return;
    const destination = pasteTargetPath(directory, clipboard.path);
    setError(null);
    try {
      if (clipboard.mode === "cut") {
        if (destination !== clipboard.path) await api.workspace.rename(clipboard.path, destination);
        setClipboard(null);
      } else if (clipboard.directory) {
        setError(toDisplayError(null, "Copying folders is not supported", "暂不支持复制文件夹"));
        return;
      } else {
        const file = await api.workspace.file(clipboard.path);
        const target = uniqueCopyPath(destination, (candidate) => Boolean(findFileNode(source, candidate)));
        await api.workspace.create(target, "file");
        await api.workspace.save(target, file.content);
        setFocusedPath(target);
      }
      await reloadTree();
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to paste", "粘贴失败"));
    }
  };

  /** 打开重命名输入栏。 */
  const beginRename = (targetPath = focusedPath) => {
    if (!targetPath) return;
    setFocusedPath(targetPath);
    setAction({ kind: "rename", value: targetPath });
    setError(null);
  };

  /** 提交当前文件操作。 */
  const submitAction = async () => {
    if (!action?.value.trim()) return;
    setError(null);
    try {
      if (action.kind === "rename" && focusedPath) {
        const entry = await api.workspace.rename(focusedPath, action.value.trim());
        if (selectedFile === focusedPath) onSelectFile(entry.path);
        setFocusedPath(entry.path);
      } else if (action.kind !== "rename") {
        const entry = await api.workspace.create(action.value.trim(), action.kind);
        setFocusedPath(entry.path);
        if (entry.kind === "file") onSelectFile(entry.path);
      }
      setAction(null);
      await reloadTree();
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to update workspace files", "更新工作区文件失败"));
    }
  };

  /** 删除当前聚焦的文件或目录。 */
  const deleteFocused = async (targetPath = focusedPath) => {
    if (!targetPath) return;
    const targetNode = findFileNode(source, targetPath);
    const confirmed = await confirm({
      title: t("Delete workspace item", "删除工作区条目"),
      description: t(`Delete “${targetPath}”${targetNode?.kind === "directory" ? " and all contents in the directory" : ""}?`, `将删除“${targetPath}”${targetNode?.kind === "directory" ? "及目录中的全部内容" : ""}。`),
      confirmLabel: t("Delete", "删除"),
      danger: true
    });
    if (!confirmed) return;
    setError(null);
    try {
      await api.workspace.remove(targetPath);
      if (selectedFile === targetPath || selectedFile?.startsWith(`${targetPath}/`)) onClearFile();
      setFocusedPath(null);
      await reloadTree();
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to delete workspace item", "删除工作区条目失败"));
    }
  };

  /** 键盘在可见行之间移动，左右键展开或收起目录。 */
  const onTreeKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const key = event.key;
    if (!["ArrowDown", "ArrowUp", "ArrowLeft", "ArrowRight", "Home", "End", "Enter"].includes(key) || rows.length === 0) return;
    event.preventDefault();
    if (focusedIndex < 0) {
      setFocusedPath(rows[key === "End" ? rows.length - 1 : 0]?.node.path ?? null);
      return;
    }
    const index = focusedIndex;
    const row = rows[index];
    const focusRow = (next: number) => {
      const target = rows[next];
      if (target) setFocusedPath(target.node.path);
    };
    const shownOpen = row?.node.kind === "directory" && (searching ? row.node.children.length > 0 : expanded.has(row.node.path));
    if (key === "ArrowDown") focusRow(Math.min(rows.length - 1, index + 1));
    else if (key === "ArrowUp") focusRow(Math.max(0, index - 1));
    else if (key === "Home") focusRow(0);
    else if (key === "End") focusRow(rows.length - 1);
    else if (key === "ArrowRight" && row?.node.kind === "directory") {
      if (!shownOpen) toggleDirectory(findFileNode(source, row.node.path) ?? row.node);
      else focusRow(Math.min(rows.length - 1, index + 1));
    } else if (key === "ArrowLeft" && row) {
      if (shownOpen && !searching) toggleDirectory(findFileNode(source, row.node.path) ?? row.node);
      else {
        const parent = parentFilePath(row.node.path);
        if (parent) setFocusedPath(parent);
      }
    }     else if (key === "Enter" && row) {
      if (row.node.kind === "directory") {
        if (!searching) toggleDirectory(findFileNode(source, row.node.path) ?? row.node);
      } else onSelectFile(row.node.path);
    }
  };

  const onScroll = (event: UIEvent<HTMLDivElement>) => {
    const top = event.currentTarget.scrollTop;
    if (scrollFrame.current) return;
    scrollFrame.current = requestAnimationFrame(() => {
      scrollFrame.current = 0;
      setScrollTop(scrollRef.current?.scrollTop ?? top);
    });
  };

  const embedded = !showHeading;

  return (
    <aside className={embedded ? "file-tree file-tree-embedded" : "file-tree"}>
      <div className="file-tree-head">
        {showHeading && <span>{t("Files", "文件")}</span>}
        {embedded && <span className="file-tree-title" title={workspaceLabel}>{workspaceLabel || t("Files", "文件")}</span>}
        <div className="file-tree-actions">
          <Button variant="ghost" size="icon" onClick={() => void reloadTree()} aria-label={t("Refresh Explorer", "刷新资源管理器")} title={t("Refresh Explorer", "刷新资源管理器")}><RefreshCw size={13} /></Button>
          <Button variant="ghost" size="icon" onClick={collapseAll} aria-label={t("Collapse All", "全部折叠")} title={t("Collapse All", "全部折叠")}><ChevronsDownUp size={13} /></Button>
          <Button variant="ghost" size="icon" onClick={() => beginCreate("file")} aria-label={t("New File", "新建文件")} title={t("New File", "新建文件")}><FilePlus2 size={13} /></Button>
          {onClose && <Button variant="ghost" size="icon" onClick={onClose} aria-label={t("Close file tree", "关闭文件树")}><PanelRightClose size={12} /></Button>}
        </div>
      </div>
      <WorkspaceFileSearch value={search} onChange={setSearch} placeholder={searchPlaceholder} />
      {showHeading && workspaceLabel && <div className="file-tree-workspace"><strong>{workspaceLabel}</strong></div>}
      <div className="file-tree-scroll">
        {action && (
          <div className="file-action-bar">
            <ChevronRight size={14} />
            <TextInput autoFocus value={action.value} onChange={(event) => setAction({ ...action, value: event.target.value })} onKeyDown={(event) => { if (event.key === "Enter") void submitAction(); if (event.key === "Escape") setAction(null); }} aria-label={t("File or directory name", "文件或目录名称")} spellCheck={false} />
            {action.kind !== "rename" && (
              <>
                <button type="button" aria-pressed={action.kind === "file"} onClick={() => setAction({ ...action, kind: "file" })} aria-label={t("New File", "新建文件")}><FilePlus2 size={13} /></button>
                <button type="button" aria-pressed={action.kind === "directory"} onClick={() => setAction({ ...action, kind: "directory" })} aria-label={t("New Folder", "新建文件夹")}><FolderPlus size={13} /></button>
              </>
            )}
          </div>
        )}
        <div
          ref={scrollRef}
          className="file-tree-rows"
          role="tree"
          tabIndex={0}
          aria-label={t("Workspace files", "工作区文件")}
          aria-activedescendant={focusedIndex >= 0 ? treeRowId(rows[focusedIndex].node.path) : undefined}
          onKeyDown={onTreeKeyDown}
          onScroll={onScroll}
          onContextMenu={(event) => {
            if ((event.target as HTMLElement).closest(".tree-row")) return;
            event.preventDefault();
            setTreeMenu({ x: event.clientX, y: event.clientY, path: "", directory: true });
          }}
        >
          <div ref={probeRef} className="tree-row-probe" aria-hidden />
          {windowRows.length > 0 && (
            <div style={{ height: rows.length * rowHeight, position: "relative" }}>
              <div style={{ position: "absolute", top: 0, left: 0, right: 0, transform: `translateY(${range.start * rowHeight}px)` }}>
                {windowRows.map((row) => (
                  <TreeRow
                    key={row.node.path}
                    node={row.node}
                    depth={row.depth}
                    open={row.node.kind === "directory" && (searching ? row.node.children.length > 0 : expanded.has(row.node.path))}
                    selected={selectedFile === row.node.path}
                    focused={focusedPath === row.node.path}
                    gitEntry={row.node.kind === "directory" ? undefined : git.entries.get(row.node.path)}
                    directoryTone={row.node.kind === "directory" ? directoryTones.get(row.node.path) : undefined}
                    onMouseDown={() => scrollRef.current?.focus({ preventScroll: true })}
                    onActivate={() => {
                      setFocusedPath(row.node.path);
                      if (row.node.kind !== "directory") {
                        onSelectFile(row.node.path);
                        return;
                      }
                      if (!searching) toggleDirectory(findFileNode(source, row.node.path) ?? row.node);
                    }}
                    onCreate={(kind) => beginCreate(kind, row.node.path)}
                    onTreeContextMenu={(event) => {
                      event.preventDefault();
                      setFocusedPath(row.node.path);
                      setTreeMenu({ x: event.clientX, y: event.clientY, path: row.node.path, directory: row.node.kind === "directory" });
                    }}
                  />
                ))}
              </div>
            </div>
          )}
          {tree.isLoading && rows.length === 0 && <p className="file-tree-empty">{t("Loading files", "正在读取文件")}</p>}
          {tree.data && rows.length === 0 && <p className="file-tree-empty">{searching ? t("No matching files", "没有匹配的文件") : t("This workspace has no files", "这个工作区没有文件")}</p>}
        </div>
        {(tree.error || error || git.error) && <p className="pane-error">{error?.message || tree.error?.message || git.error?.message}</p>}
      </div>
      {treeMenu && (
        <FileTreeContextMenu
          x={treeMenu.x}
          y={treeMenu.y}
          path={treeMenu.path}
          directory={treeMenu.directory}
          canPaste={Boolean(clipboard)}
          onOpenContaining={() => revealContaining(treeMenu.path)}
          onCreate={(kind) => beginCreate(kind, treeMenu.path)}
          onCopyPath={() => void navigator.clipboard?.writeText(absoluteWorkspacePath(workspaceRoot, treeMenu.path))}
          onCopyRelativePath={() => void navigator.clipboard?.writeText(treeMenu.path)}
          onCut={() => setClipboard({ mode: "cut", path: treeMenu.path, directory: treeMenu.directory })}
          onCopy={() => setClipboard({ mode: "copy", path: treeMenu.path, directory: treeMenu.directory })}
          onPaste={() => void pasteInto(treeMenu.directory ? treeMenu.path : parentFilePath(treeMenu.path))}
          onRename={() => beginRename(treeMenu.path)}
          onDelete={() => void deleteFocused(treeMenu.path)}
          onClose={() => setTreeMenu(null)}
        />
      )}
    </aside>
  );
}

/** 渲染虚拟列表中的一行。 */
function TreeRow({ node, depth, open, selected, focused, gitEntry, directoryTone, onMouseDown, onActivate, onCreate, onTreeContextMenu }: {
  node: FileNode;
  depth: number;
  open: boolean;
  selected: boolean;
  focused: boolean;
  gitEntry?: FileTreeGitEntry;
  directoryTone?: FileTreeGitTone;
  onMouseDown: () => void;
  onActivate: () => void;
  onCreate: (kind: "file" | "directory") => void;
  onTreeContextMenu: (event: MouseEvent<HTMLDivElement>) => void;
}) {
  const { t } = useI18n();
  const directory = node.kind === "directory";
  const className = ["tree-row", selected ? "active" : "", focused ? "focused" : ""].filter(Boolean).join(" ");
  return (
    <div
      id={treeRowId(node.path)}
      role="treeitem"
      aria-expanded={directory ? open : undefined}
      aria-selected={selected}
      aria-level={depth + 1}
      className={className}
      style={{ paddingLeft: TREE_PAD + depth * TREE_INDENT }}
      title={node.path}
      onMouseDown={(event) => {
        if (event.button !== 0) return;
        event.preventDefault();
        onMouseDown();
      }}
      onClick={onActivate}
      onKeyDown={(event) => {
        if (event.key === "Enter") onActivate();
      }}
      onContextMenu={onTreeContextMenu}
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
          <button type="button" onClick={(event) => { event.stopPropagation(); onCreate("file"); }} aria-label={t("New File", "新建文件")} title={t("New File", "新建文件")}><FilePlus2 size={13} /></button>
          <button type="button" onClick={(event) => { event.stopPropagation(); onCreate("directory"); }} aria-label={t("New Folder", "新建文件夹")} title={t("New Folder", "新建文件夹")}><FolderPlus size={13} /></button>
        </span>
      )}
      {gitEntry && <span className={`tree-row-git-status git-${fileTreeGitStatusTone(gitEntry.entry)}`}>{fileTreeGitStatusLabel(gitEntry.entry)}</span>}
    </div>
  );
}

/** 把路径收成合法的 DOM id。 */
function treeRowId(path: string): string {
  return `file-tree-row-${encodeURIComponent(path)}`;
}

/** 刷新文件树、文件内容和 Git 状态。 */
async function refreshWorkspaceQueries(queryClient: ReturnType<typeof useQueryClient>): Promise<void> {
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: ["file-tree"] }),
    queryClient.invalidateQueries({ queryKey: ["file"] }),
    queryClient.invalidateQueries({ queryKey: ["workspace-diff"] })
  ]);
}
