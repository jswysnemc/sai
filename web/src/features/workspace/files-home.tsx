import { useQuery, useQueryClient } from "@tanstack/react-query";
import { FilePlus2, FolderSearch } from "lucide-react";
import { useMemo, useState } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import { FileTypeIcon } from "../../shared/ui/file-icon";
import { useI18n } from "../i18n/use-i18n";
import { directoryPaths, flattenVisibleNodes, parentFilePath } from "./file-tree-utils";
import { WorkspaceFileSearch } from "./workspace-file-search";
import "./files-home.css";

/** 让已经打开的文件树展开并聚焦这个路径。 */
export const REVEAL_FILE_TREE_PATH_EVENT = "sai:reveal-file-tree-path";

type FilesHomeProps = {
  recentFiles: string[];
  onSelectFile: (path: string) => void;
  onBrowseFiles: () => void;
};

/**
 * 渲染未打开文件时的 Files 面板：搜索、浏览、新建和最近文件。
 *
 * @param props 最近文件与打开回调
 * @returns Files 首页
 */
export function FilesHome({ recentFiles, onSelectFile, onBrowseFiles }: FilesHomeProps) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const tree = useQuery({ queryKey: ["file-tree"], queryFn: () => api.workspace.tree(), staleTime: 15_000 });
  const [query, setQuery] = useState("");
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const rows = useMemo(() => flattenVisibleNodes(tree.data ?? [], directoryPaths(tree.data ?? [])), [tree.data]);
  const needle = query.trim().toLocaleLowerCase();
  const matches = needle
    ? rows.filter((row) => row.node.name.toLocaleLowerCase().includes(needle) || row.node.path.toLocaleLowerCase().includes(needle)).slice(0, 30)
    : [];
  const recents = recentFiles.map((path) => ({ path, name: path.split("/").pop() ?? path }));

  /** 在工作区根创建文件并打开。 */
  const createFile = async () => {
    const path = name.trim().replace(/^\/+/, "");
    if (!path) return;
    setError(null);
    try {
      const entry = await api.workspace.create(path, "file");
      await queryClient.invalidateQueries({ queryKey: ["file-tree"] });
      setCreating(false);
      setName("");
      onSelectFile(entry.path);
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to create file", "新建文件失败").message);
    }
  };

  /** 打开文件，或让文件树展开目录。 */
  const openRow = (path: string, directory: boolean) => {
    if (directory) {
      onBrowseFiles();
      window.dispatchEvent(new CustomEvent(REVEAL_FILE_TREE_PATH_EVENT, { detail: path }));
      return;
    }
    onSelectFile(path);
  };

  return (
    <div className="files-home">
      <WorkspaceFileSearch value={query} onChange={setQuery} placeholder={t("Files, Folders...", "文件、文件夹...")} />
      <div className="files-home-actions">
        <button type="button" onClick={onBrowseFiles}><FolderSearch size={15} />{t("Browse Files", "浏览文件")}</button>
        <button type="button" onClick={() => setCreating(true)}><FilePlus2 size={15} />{t("New File", "新建文件")}</button>
      </div>
      {creating && (
        <form className="files-home-create" onSubmit={(event) => { event.preventDefault(); void createFile(); }}>
          <input autoFocus value={name} onChange={(event) => setName(event.target.value)} onKeyDown={(event) => { if (event.key === "Escape") setCreating(false); }} placeholder={t("File name", "文件名")} aria-label={t("File name", "文件名")} spellCheck={false} />
        </form>
      )}
      {error && <p className="files-home-error">{error}</p>}
      <section className="files-home-list" aria-label={needle ? t("Matching files", "匹配的文件") : t("Recents", "最近")}>
        <h2>{needle ? t("Results", "结果") : t("Recents", "最近")}</h2>
        {(needle ? matches.map((row) => ({ path: row.node.path, name: row.node.name, directory: row.node.kind === "directory" })) : recents.map((item) => ({ ...item, directory: false }))).map((item) => (
          <button key={item.path} type="button" className="files-home-row" onClick={() => openRow(item.path, item.directory)}>
            <FileTypeIcon name={item.name} size={16} />
            <strong>{item.name}</strong>
            {parentFilePath(item.path) && <span>{parentFilePath(item.path)}</span>}
          </button>
        ))}
        {!needle && recents.length === 0 && <p>{t("Files you open will show up here", "打开过的文件会显示在这里")}</p>}
        {needle && matches.length === 0 && <p>{t("No matching files", "没有匹配的文件")}</p>}
      </section>
    </div>
  );
}
