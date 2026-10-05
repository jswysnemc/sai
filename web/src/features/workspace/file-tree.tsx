import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronRight, Eye, FilePlus2, FolderPlus, MoreHorizontal, Search } from "../../shared/ui/icons";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type DragEvent, type KeyboardEvent, type UIEvent } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import type { FileNode } from "../../api/contracts";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { readExpandedDirectories, writeExpandedDirectories } from "./file-tree-expansion";
import { readShowHiddenFiles, writeShowHiddenFiles } from "./file-tree-visibility";
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
import { ActionMenu } from "../../shared/ui/menu/action-menu";
import { TextInput } from "../../shared/ui/form/text-input";
import { absoluteWorkspacePath, pasteTargetPath, uniqueCopyPath, type TreeClipboard } from "./file-tree-clipboard";
import { dropDirectory, droppedFiles, isNestedDrop, readInternalDrag, setInternalDrag } from "./file-tree-drag";
import { REVEAL_FILE_TREE_PATH_EVENT } from "./files-home";
import { FileTreeContextMenu } from "./file-tree-context-menu";
import { FileTreeSelectionBar } from "./file-tree-selection-bar";
import { outermostPaths } from "./file-tree-selection";
import { useFileTreeSelection } from "./use-file-tree-selection";
import { TreeRow, treeRowId } from "./file-tree-row";
import { directoryGitTones, useFileTreeGit } from "./use-file-tree-git";

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
type TreeMenuState = { x: number; y: number; path: string; directory: boolean; paths: string[] } | null;

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
  const [showHidden, setShowHidden] = useState(readShowHiddenFiles);
  const treeScope = JSON.stringify([storageKey, showHidden]);
  const tree = useQuery({ queryKey: ["file-tree", storageKey, showHidden], queryFn: () => api.workspace.tree("", 5, showHidden), refetchOnWindowFocus: true, refetchInterval: 15_000 });
  const [treeScopeSeen, setTreeScopeSeen] = useState(treeScope);
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(() => withAncestors(readExpandedDirectories(storageKey), selectedFile));
  const [lazyChildren, setLazyChildren] = useState<ReadonlyMap<string, FileNode[]>>(() => new Map());
  const [focusedPath, setFocusedPath] = useState<string | null>(selectedFile);
  const [action, setAction] = useState<FileAction>(null);
  const [search, setSearch] = useState("");
  const [searchOpen, setSearchOpen] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [treeMenu, setTreeMenu] = useState<TreeMenuState>(null);
  const [clipboard, setClipboard] = useState<TreeClipboard | null>(null);
  const [dropTarget, setDropTarget] = useState<string | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewport, setViewport] = useState(0);
  const [rowHeight, setRowHeight] = useState(22);
  const scrollRef = useRef<HTMLDivElement>(null);
  const probeRef = useRef<HTMLDivElement>(null);
  const scrollFrame = useRef(0);
  const inFlight = useRef(new Set<string>());
  const activeTreeScope = useRef(treeScope);
  const lazyPathsRef = useRef(lazyChildren);
  const expandedRef = useRef(expanded);
  const pinnedPath = useRef<string | null>(null);
  lazyPathsRef.current = lazyChildren;
  expandedRef.current = expanded;
  activeTreeScope.current = treeScope;

  if (treeScopeSeen !== treeScope) {
    setTreeScopeSeen(treeScope);
    setExpanded(withAncestors(readExpandedDirectories(storageKey), selectedFile));
    setLazyChildren(new Map());
    inFlight.current = new Set();
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
  const rowOrder = useMemo(() => rows.map((row) => row.node.path), [rows]);
  const pathExists = useCallback((path: string) => Boolean(findFileNode(source, path)), [source]);
  const multi = useFileTreeSelection(rowOrder, pathExists);
  const selectedCount = multi.selection.paths.size;
  const range = visibleRowRange(rows.length, scrollTop, viewport, rowHeight);
  const windowRows = rows.slice(range.start, range.end);

  /**
   * 【工作区】【文件树】按当前显示选项读取目录，丢弃切换前的异步结果。
   * @param path 待展开的工作区相对目录
   * @returns 目录加载完成，无返回值
   */
  const loadDirectory = useCallback(async (path: string) => {
    const requests = inFlight.current;
    if (treeScope !== activeTreeScope.current || requests.has(path)) return;
    requests.add(path);
    try {
      const children = await api.workspace.tree(path, 2, showHidden);
      if (requests !== inFlight.current) return;
      setLazyChildren((current) => {
        const next = new Map(current);
        next.set(path, children);
        return next;
      });
      setError(null);
    } catch (reason) {
      if (requests === inFlight.current) setError(toDisplayError(reason, "Failed to read directory", "读取目录失败"));
    } finally {
      requests.delete(path);
    }
  }, [showHidden, treeScope]);

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
    // 1. 【工作区】【文件树】切换显示选项后，补读仍处于展开状态的深层目录
    for (const path of expanded) {
      const node = findFileNode(source, path);
      if (node?.kind === "directory" && node.children.length === 0 && !lazyChildren.has(path)) void loadDirectory(path);
    }
  }, [source, expanded, lazyChildren, loadDirectory]);

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

  /**
   * 切换隐藏条目显示状态，并记住选择。
   * @returns 无返回值
   */
  const toggleHiddenFiles = () => {
    const next = !showHidden;
    setShowHidden(next);
    writeShowHiddenFiles(next);
    setTreeMenu(null);
    setError(null);
    setScrollTop(0);
    if (scrollRef.current) scrollRef.current.scrollTop = 0;
  };

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

  /**
   * 把内部路径或外部文件落到目标目录。
   * @param directory 落点目录
   * @param event 拖放事件
   * @returns 完成后的 Promise
   */
  const acceptDrop = async (directory: string, event: DragEvent<HTMLElement>) => {
    event.preventDefault();
    setDropTarget(null);
    const internal = readInternalDrag(event);
    if (internal) {
      if (isNestedDrop(internal, directory)) return;
      const destination = pasteTargetPath(directory, internal);
      if (destination === internal) return;
      setError(null);
      try {
        await api.workspace.rename(internal, destination);
        setFocusedPath(destination);
        await reloadTree();
      } catch (reason) {
        setError(toDisplayError(reason, "Failed to move", "移动失败"));
      }
      return;
    }
    const files = droppedFiles(event);
    if (!files.length) return;
    setError(null);
    try {
      for (const file of files) {
        const relative = directory ? `${directory}/${file.name}` : file.name;
        const target = uniqueCopyPath(relative, (candidate) => Boolean(findFileNode(source, candidate)));
        const content = await file.text();
        await api.workspace.create(target, "file");
        await api.workspace.save(target, content);
        setFocusedPath(target);
      }
      await reloadTree();
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to drop files", "拖入文件失败"));
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

  /**
   * 【工作区】【文件树多选】确认后依次删除多个条目，被已选目录包含的子项不重复删除。
   * @param paths 待删除的路径
   * @returns 完成后的 Promise
   */
  const deleteMany = async (paths: string[]) => {
    const targets = outermostPaths(paths);
    if (targets.length === 0) return;
    if (targets.length === 1) return deleteFocused(targets[0]);
    const confirmed = await confirm({
      title: t("Delete workspace items", "删除工作区条目"),
      description: t(`Delete ${targets.length} items, including all contents of selected folders?`, `将删除 ${targets.length} 项，所选目录中的内容一并删除。`),
      confirmLabel: t("Delete", "删除"),
      danger: true
    });
    if (!confirmed) return;
    setError(null);
    try {
      for (const target of targets) {
        await api.workspace.remove(target);
        if (selectedFile === target || selectedFile?.startsWith(`${target}/`)) onClearFile();
      }
      multi.clear();
      setFocusedPath(null);
      await reloadTree();
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to delete workspace items", "删除工作区条目失败"));
      await reloadTree();
    }
  };

  /** 把多个路径按行写入剪贴板；absolute 为 true 时写入绝对路径。 */
  const copyPaths = (paths: string[], absolute: boolean) => {
    const text = paths.map((path) => absolute ? absoluteWorkspacePath(workspaceRoot, path) : path).join("\n");
    void navigator.clipboard?.writeText(text);
  };

  /** 键盘在可见行之间移动，左右键展开或收起目录。 */
  const onTreeKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const key = event.key;
    // 1. 多选快捷键：Esc 取消、Ctrl/⌘+A 全选可见行、Delete 删除所选
    if (key === "Escape" && selectedCount > 0) { event.preventDefault(); multi.clear(); return; }
    if ((event.ctrlKey || event.metaKey) && key.toLowerCase() === "a" && rows.length > 0) { event.preventDefault(); multi.selectAll(); return; }
    if (key === "Delete" && (selectedCount > 0 || focusedPath)) {
      event.preventDefault();
      void deleteMany(selectedCount > 0 ? [...multi.selection.paths] : [focusedPath ?? ""]);
      return;
    }
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
      if (!target) return;
      setFocusedPath(target.node.path);
      // Shift+方向键从锚点扩展区间；无锚点时以当前行起算
      if (event.shiftKey) {
        if (multi.selection.anchor === null) multi.click(row.node.path, { toggle: false, range: false });
        multi.click(target.node.path, { toggle: false, range: true });
      }
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
          <Button className="file-tree-search-toggle" variant="ghost" size="icon" aria-pressed={showHidden} onClick={toggleHiddenFiles} aria-label={t("Show hidden files", "显示隐藏文件")} title={showHidden ? t("Hide hidden files", "不显示隐藏文件") : t("Show hidden files", "显示隐藏文件")}><Eye size={14} /></Button>
          <Button className="file-tree-search-toggle" variant="ghost" size="icon" aria-pressed={searchOpen || Boolean(search)} onClick={() => setSearchOpen((open) => !open || Boolean(search))} aria-label={t("Filter files", "筛选文件")} title={t("Filter files", "筛选文件")}><Search size={14} /></Button>
          <ActionMenu
            label={t("File tree actions", "文件树操作")}
            trigger={<MoreHorizontal size={14} />}
            triggerClassName="file-tree-more"
            items={[
              { id: "refresh", label: t("Refresh Explorer", "刷新资源管理器"), onSelect: () => void reloadTree() },
              { id: "collapse", label: t("Collapse All", "全部折叠"), onSelect: collapseAll },
              { id: "new-file", label: t("New File", "新建文件"), onSelect: () => beginCreate("file") },
              ...(onClose ? [{ id: "close", label: t("Close file tree", "关闭文件树"), separator: true, onSelect: onClose }] : [])
            ]}
          />
        </div>
      </div>
      {(searchOpen || search) && (
        <WorkspaceFileSearch value={search} onChange={setSearch} placeholder={searchPlaceholder} autoFocus />
      )}
      {showHeading && workspaceLabel && <div className="file-tree-workspace"><strong>{workspaceLabel}</strong></div>}
      <div className="file-tree-scroll">
        {action && (
          <div className="file-action-bar">
            <ChevronRight size={14} />
            <TextInput autoFocus value={action.value} onChange={(event) => setAction({ ...action, value: event.target.value })} onKeyDown={(event) => { if (event.key === "Enter") void submitAction(); if (event.key === "Escape") setAction(null); }} aria-label={t("File or directory name", "文件或目录名称")} spellCheck={false} />
            {action.kind !== "rename" && (
              <>
                <button type="button" aria-pressed={action.kind === "file"} onClick={() => setAction({ ...action, kind: "file" })} aria-label={t("New File", "新建文件")}><FilePlus2 size={14} /></button>
                <button type="button" aria-pressed={action.kind === "directory"} onClick={() => setAction({ ...action, kind: "directory" })} aria-label={t("New Folder", "新建文件夹")}><FolderPlus size={14} /></button>
              </>
            )}
          </div>
        )}
        {selectedCount > 1 && (
          <FileTreeSelectionBar
            count={selectedCount}
            onCopyPaths={() => copyPaths([...multi.selection.paths], false)}
            onDelete={() => void deleteMany([...multi.selection.paths])}
            onClear={multi.clear}
          />
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
          onDragOver={(event) => { event.preventDefault(); setDropTarget(""); }}
          onDragLeave={() => setDropTarget((current) => current === "" ? null : current)}
          onDrop={(event) => void acceptDrop("", event)}
          onContextMenu={(event) => {
            if ((event.target as HTMLElement).closest(".tree-row")) return;
            event.preventDefault();
            multi.clear();
            setTreeMenu({ x: event.clientX, y: event.clientY, path: "", directory: true, paths: [] });
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
                    multiSelected={selectedCount > 1 && multi.selection.paths.has(row.node.path)}
                    focused={focusedPath === row.node.path}
                    dropActive={dropTarget === row.node.path || (dropTarget === "" && row.node.path === "")}
                    gitEntry={row.node.kind === "directory" ? undefined : git.entries.get(row.node.path)}
                    directoryTone={row.node.kind === "directory" ? directoryTones.get(row.node.path) : undefined}
                    onMouseDown={() => scrollRef.current?.focus({ preventScroll: true })}
                    onDragStart={(event) => setInternalDrag(event, row.node.path)}
                    onDragOver={(event) => {
                      event.preventDefault();
                      event.stopPropagation();
                      setDropTarget(dropDirectory(row.node.path, row.node.kind === "directory"));
                    }}
                    onDrop={(event) => {
                      event.stopPropagation();
                      void acceptDrop(dropDirectory(row.node.path, row.node.kind === "directory"), event);
                    }}
                    onActivate={(modifiers) => {
                      setFocusedPath(row.node.path);
                      multi.click(row.node.path, modifiers);
                      // Ctrl/⌘ 或 Shift 只调整选择，不打开文件、不展开目录
                      if (modifiers.toggle || modifiers.range) return;
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
                      const paths = multi.context(row.node.path);
                      setTreeMenu({ x: event.clientX, y: event.clientY, path: row.node.path, directory: row.node.kind === "directory", paths });
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
          selectedCount={treeMenu.paths.length}
          onOpenContaining={() => revealContaining(treeMenu.path)}
          onCreate={(kind) => beginCreate(kind, treeMenu.path)}
          onCopyPath={() => copyPaths(treeMenu.paths.length > 1 ? treeMenu.paths : [treeMenu.path], true)}
          onCopyRelativePath={() => copyPaths(treeMenu.paths.length > 1 ? treeMenu.paths : [treeMenu.path], false)}
          onCut={() => setClipboard({ mode: "cut", path: treeMenu.path, directory: treeMenu.directory })}
          onCopy={() => setClipboard({ mode: "copy", path: treeMenu.path, directory: treeMenu.directory })}
          onPaste={() => void pasteInto(treeMenu.directory ? treeMenu.path : parentFilePath(treeMenu.path))}
          onRename={() => beginRename(treeMenu.path)}
          onDelete={() => void (treeMenu.paths.length > 1 ? deleteMany(treeMenu.paths) : deleteFocused(treeMenu.path))}
          onClose={() => setTreeMenu(null)}
        />
      )}
    </aside>
  );
}

/** 刷新文件树、文件内容和 Git 状态。 */
async function refreshWorkspaceQueries(queryClient: ReturnType<typeof useQueryClient>): Promise<void> {
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: ["file-tree"] }),
    queryClient.invalidateQueries({ queryKey: ["file"] }),
    queryClient.invalidateQueries({ queryKey: ["workspace-diff"] })
  ]);
}
