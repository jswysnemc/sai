import { useQuery } from "@tanstack/react-query";
import { ChevronDown, ChevronRight, Folder, FolderOpen } from "lucide-react";
import { useMemo, useRef, useState } from "react";
import { api } from "../../api/client";
import type { FileNode } from "../../api/contracts";
import { useOutsidePointerDown } from "../../shared/hooks/use-outside-pointer-down";
import { FileTypeIcon } from "../../shared/ui/file-icon";
import { breadcrumbDirectoryPath, buildBreadcrumbParts } from "./editor-breadcrumb-utils";
import { workspaceRelativePath } from "./workspace-path-utils";
import "./editor-breadcrumbs.css";
import { useI18n } from "../i18n/use-i18n";

type EditorBreadcrumbsProps = {
  path: string;
  onSelectFile: (path: string) => void;
};

/**
 * 渲染可展开目录内容和同级文件的编辑器面包屑。
 *
 * @param props 当前文件路径和文件打开回调
 * @returns 编辑器路径导航
 */
export function EditorBreadcrumbs({ path, onSelectFile }: EditorBreadcrumbsProps) {
  const { t } = useI18n();
  const rootRef = useRef<HTMLElement>(null);
  const [openPath, setOpenPath] = useState<string | null>(null);
  const tree = useQuery({ queryKey: ["file-tree"], queryFn: () => api.workspace.tree() });
  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: api.workspaces.list });
  const workspace = workspaces.data?.workspaces.find((item) => item.id === workspaces.data.active_id);
  const workspaceName = workspace?.name ?? t("Workspace", "工作区");
  const nodes = tree.data ?? [];
  const relativePath = useMemo(() => workspaceRelativePath(path, workspace?.path ?? ""), [path, workspace?.path]);
  const parts = useMemo(() => buildBreadcrumbParts(relativePath, nodes, workspaceName), [relativePath, nodes, workspaceName]);
  const openPart = parts.find((part) => part.path === openPath) ?? null;
  const menuDirectory = breadcrumbDirectoryPath(openPart);
  const directory = useQuery({
    queryKey: ["breadcrumb-directory", menuDirectory],
    queryFn: () => api.workspace.tree(menuDirectory ?? "", 5),
    enabled: menuDirectory !== null
  });
  const menuNodes = directory.data ?? [];
  useOutsidePointerDown(rootRef, () => setOpenPath(null), openPath !== null);

  return (
    <nav className="editor-breadcrumbs" aria-label={t("Current file path", "当前文件路径")} ref={rootRef}>
      <div className="editor-breadcrumb-list">
        {parts.map((part, index) => (
          <span className="editor-breadcrumb-part" key={`${part.kind}:${part.path}`}>
            {index > 0 && <ChevronRight size={12} className="editor-breadcrumb-separator" aria-hidden="true" />}
            <button
              type="button"
              className={openPath === part.path ? "active" : ""}
              onClick={() => setOpenPath((current) => current === part.path ? null : part.path)}
              title={part.path || workspaceName}
              aria-expanded={openPath === part.path}
            >
              <span>{part.label}</span>
              <ChevronDown size={10} aria-hidden="true" />
            </button>
          </span>
        ))}
      </div>
      {openPart && (
        <div className="editor-breadcrumb-menu">
          {directory.isLoading ? <span className="editor-breadcrumb-empty">{t("Loading directory", "正在读取目录")}</span> : menuNodes.length > 0 ? menuNodes.map((node) => (
            <BreadcrumbMenuItem key={node.path} node={node} onSelectFile={onSelectFile} onClose={() => setOpenPath(null)} depth={0} />
          )) : <span className="editor-breadcrumb-empty">{t("The directory has no displayable files", "目录中没有可显示的文件")}</span>}
        </div>
      )}
    </nav>
  );
}

/**
 * 渲染面包屑下拉中的文件或目录条目。
 *
 * @param props 文件节点、打开文件回调和关闭菜单回调
 * @returns 面包屑下拉条目
 */
function BreadcrumbMenuItem({ node, onSelectFile, onClose, depth }: { node: FileNode; onSelectFile: (path: string) => void; onClose: () => void; depth: number }) {
  const directory = node.kind === "directory";
  const [open, setOpen] = useState(false);
  return (
    <div>
      <button
        type="button"
        className="editor-breadcrumb-menu-item"
        style={{ paddingLeft: `${7 + depth * 15}px` }}
        onClick={() => {
          if (directory) setOpen((value) => !value);
          else {
            onSelectFile(node.path);
            onClose();
          }
        }}
        title={node.path}
      >
        {directory ? <ChevronRight size={12} className={open ? "editor-breadcrumb-folder-chevron open" : "editor-breadcrumb-folder-chevron"} /> : <span className="editor-breadcrumb-file-spacer" />}
        {directory ? (open ? <FolderOpen size={14} /> : <Folder size={14} />) : <FileTypeIcon name={node.name} size={14} />}
        <span>{node.name}</span>
      </button>
      {directory && open && node.children.map((child) => (
        <BreadcrumbMenuItem key={child.path} node={child} onSelectFile={onSelectFile} onClose={onClose} depth={depth + 1} />
      ))}
    </div>
  );
}
