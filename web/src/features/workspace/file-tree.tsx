import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, ChevronRight, FilePlus2, Folder, FolderOpen, FolderPlus, PanelRightClose, Pencil, RefreshCw, Trash2, X } from "lucide-react";
import { useState } from "react";
import { api } from "../../api/client";
import type { FileNode } from "../../api/contracts";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { FileTypeIcon } from "../../shared/ui/file-icon";
import { filterFileNodes, findFileNode, parentFilePath } from "./file-tree-utils";
import { WorkspaceFileSearch } from "./workspace-file-search";
import { useI18n } from "../i18n/use-i18n";

type FileTreeProps = {
  selectedFile: string | null;
  onSelectFile: (path: string) => void;
  onClearFile: () => void;
  onClose: () => void;
};

type FileAction = { kind: "file" | "directory" | "rename"; value: string } | null;

/**
 * 渲染支持创建、重命名和删除的工作区文件树。
 *
 * @param props 当前文件选择、更新回调与关闭文件树回调
 * @returns 文件浏览器
 */
export function FileTree({ selectedFile, onSelectFile, onClearFile, onClose }: FileTreeProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const tree = useQuery({ queryKey: ["file-tree"], queryFn: () => api.workspace.tree(), refetchOnWindowFocus: true, refetchInterval: 15_000 });
  const [focusedPath, setFocusedPath] = useState<string | null>(selectedFile);
  const [action, setAction] = useState<FileAction>(null);
  const [search, setSearch] = useState("");
  const [error, setError] = useState("");
  const focusedNode = findFileNode(tree.data ?? [], focusedPath);
  const visibleNodes = filterFileNodes(tree.data ?? [], search);

  /** 打开新建文件或目录输入栏。 */
  const beginCreate = (kind: "file" | "directory") => {
    const parent = focusedNode?.kind === "directory" ? focusedNode.path : parentFilePath(focusedPath ?? "");
    setAction({ kind, value: parent ? `${parent}/` : "" });
    setError("");
  };

  /** 打开重命名输入栏。 */
  const beginRename = () => {
    if (!focusedPath) return;
    setAction({ kind: "rename", value: focusedPath });
    setError("");
  };

  /** 提交当前文件操作。 */
  const submitAction = async () => {
    if (!action?.value.trim()) return;
    setError("");
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
      await refreshWorkspaceQueries(queryClient);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  };

  /** 删除当前聚焦的文件或目录。 */
  const deleteFocused = async () => {
    if (!focusedPath) return;
    const confirmed = await confirm({
      title: t("Delete workspace item", "删除工作区条目"),
      description: t(`Delete “${focusedPath}”${focusedNode?.kind === "directory" ? " and all contents in the directory" : ""}?`, `将删除“${focusedPath}”${focusedNode?.kind === "directory" ? "及目录中的全部内容" : ""}。`),
      confirmLabel: t("Delete", "删除"),
      danger: true
    });
    if (!confirmed) return;
    setError("");
    try {
      await api.workspace.remove(focusedPath);
      if (selectedFile === focusedPath || selectedFile?.startsWith(`${focusedPath}/`)) onClearFile();
      setFocusedPath(null);
      await refreshWorkspaceQueries(queryClient);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  };

  return (
    <aside className="file-tree">
      <div className="file-tree-head">
        <span>{t("Files", "文件")}</span>
        <div className="file-tree-actions">
          <button type="button" onClick={() => beginCreate("file")} aria-label={t("New file", "新建文件")}><FilePlus2 size={13} /></button>
          <button type="button" onClick={() => beginCreate("directory")} aria-label={t("New directory", "新建目录")}><FolderPlus size={13} /></button>
          <button type="button" onClick={beginRename} disabled={!focusedPath} aria-label={t("Rename", "重命名")}><Pencil size={12} /></button>
          <button type="button" onClick={() => void deleteFocused()} disabled={!focusedPath} aria-label={t("Delete", "删除")}><Trash2 size={12} /></button>
          <button type="button" onClick={() => void tree.refetch()} aria-label={t("Refresh file tree", "刷新文件树")}><RefreshCw size={12} /></button>
          <button type="button" onClick={onClose} aria-label={t("Close file tree", "关闭文件树")}><PanelRightClose size={12} /></button>
        </div>
      </div>
      <WorkspaceFileSearch value={search} onChange={setSearch} />
      <div className="file-tree-scroll">
        {action && (
          <div className="file-action-bar">
            {action.kind === "directory" ? <FolderPlus size={13} /> : action.kind === "file" ? <FilePlus2 size={13} /> : <Pencil size={13} />}
            <input autoFocus value={action.value} onChange={(event) => setAction({ ...action, value: event.target.value })} onKeyDown={(event) => { if (event.key === "Enter") void submitAction(); if (event.key === "Escape") setAction(null); }} spellCheck={false} />
            <button type="button" onClick={() => void submitAction()} aria-label={t("Confirm", "确认")}><Check size={12} /></button>
            <button type="button" onClick={() => setAction(null)} aria-label={t("Cancel", "取消")}><X size={12} /></button>
          </div>
        )}
        {visibleNodes.map((node) => <TreeNode key={node.path} node={node} selectedFile={selectedFile} focusedPath={focusedPath} onFocus={setFocusedPath} onSelectFile={onSelectFile} depth={0} forceOpen={Boolean(search.trim())} />)}
        {tree.data && visibleNodes.length === 0 && <p className="file-tree-empty">{t("No matching files", "没有匹配的文件")}</p>}
        {(tree.error || error) && <p className="pane-error">{error || tree.error?.message}</p>}
      </div>
    </aside>
  );
}

/** 渲染单个递归文件树节点。 */
function TreeNode({ node, selectedFile, focusedPath, onFocus, onSelectFile, depth, forceOpen }: { node: FileNode; selectedFile: string | null; focusedPath: string | null; onFocus: (path: string) => void; onSelectFile: (path: string) => void; depth: number; forceOpen: boolean }) {
  const [open, setOpen] = useState(depth < 1);
  const directory = node.kind === "directory";
  const active = selectedFile === node.path || focusedPath === node.path;
  return (
    <div>
      <button
        type="button"
        className={active ? "tree-row active" : "tree-row"}
        style={{ paddingLeft: `${8 + depth * 13}px` }}
        onClick={() => {
          onFocus(node.path);
          if (directory) setOpen((value) => !value);
          else onSelectFile(node.path);
        }}
      >
        {directory ? <ChevronRight size={12} className={open ? "tree-chevron open" : "tree-chevron"} /> : <span className="tree-spacer" />}
        {directory ? (open ? <FolderOpen size={14} /> : <Folder size={14} />) : <FileTypeIcon name={node.name} size={13} />}
        <span>{node.name}</span>
      </button>
      {directory && (open || forceOpen) && node.children.map((child) => <TreeNode key={child.path} node={child} selectedFile={selectedFile} focusedPath={focusedPath} onFocus={onFocus} onSelectFile={onSelectFile} depth={depth + 1} forceOpen={forceOpen} />)}
    </div>
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
