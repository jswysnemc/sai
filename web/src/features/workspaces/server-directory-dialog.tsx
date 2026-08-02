import { ArrowUp, Check, CornerDownLeft, Eye, EyeOff, Folder, FolderPlus, FolderSearch, GitBranch, HardDrive } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import type { DirectoryEntry } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { Modal } from "../../shared/ui/dialog/modal";
import { useI18n } from "../i18n/use-i18n";
import { isAbsoluteFilesystemPath, normalizePathInput } from "./path-utils";
import { parsePickedDirectory } from "./picked-directory";

type ServerDirectoryDialogProps = {
  open: boolean;
  title?: string;
  description?: string;
  selectedLabel?: string;
  currentLabel?: string;
  pendingLabel?: string;
  onClose: () => void;
  onSelect: (path: string) => Promise<void>;
};

/**
 * 渲染服务端目录浏览和工作区选择对话框。
 *
 * @param props 打开状态、关闭回调和目录选择回调
 * @returns 服务端目录选择弹层
 */
export function ServerDirectoryDialog(props: ServerDirectoryDialogProps) {
  const { t } = useI18n();
  const [path, setPath] = useState<string | undefined>();
  const [draft, setDraft] = useState("");
  const [selected, setSelected] = useState("");
  const [showHidden, setShowHidden] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [creating, setCreating] = useState(false);
  const [newFolderName, setNewFolderName] = useState("");
  const [createError, setCreateError] = useState<Error | null>(null);
  const [submitError, setSubmitError] = useState<Error | null>(null);
  const [pickError, setPickError] = useState<string | null>(null);
  const pickerRef = useRef<HTMLInputElement>(null);
  const listing = useQuery({ queryKey: ["workspace-directories", path], queryFn: () => api.workspaces.browse(path), enabled: props.open });
  const filter = isAbsoluteFilesystemPath(draft) ? "" : draft.trim();
  const entries = useMemo(
    () => filterEntries(sortEntries(listing.data?.entries ?? [], showHidden), filter),
    [listing.data?.entries, showHidden, filter]
  );
  const hiddenCount = (listing.data?.entries.length ?? 0) - sortEntries(listing.data?.entries ?? [], false).length;

  useEffect(() => {
    if (props.open) setSubmitError(null);
  }, [props.open]);

  /** 切换当前浏览目录并清空过滤与选中状态。 */
  const navigate = (nextPath: string) => {
    setPath(nextPath);
    setDraft("");
    setSelected("");
    setCreating(false);
    setCreateError(null);
    setSubmitError(null);
    setPickError(null);
  };

  /**
   * 处理浏览器目录选择的结果。
   *
   * 浏览器只交出相对路径，因此把选中的目录名拼到各允许根之下，
   * 逐个探测哪一个在服务端真实存在，命中后直接跳转过去。
   *
   * @param files 目录选择器返回的文件列表
   * @returns 无返回值
   */
  const handlePickedDirectory = async (files: File[]) => {
    setPickError(null);
    const roots = (listing.data?.roots ?? []).map((root) => root.path);
    const picked = parsePickedDirectory(
      files.map((file) => file.webkitRelativePath || file.name),
      roots
    );
    if (!picked.name) return;
    // 1. 逐个候选探测，命中第一个能列出内容的路径
    for (const candidate of picked.candidates) {
      try {
        await api.workspaces.browse(candidate);
        navigate(candidate);
        setSelected(candidate);
        return;
      } catch {
        // 该根下没有同名目录，继续试下一个
      }
    }
    // 2. 全部落空：多半是选了允许根之外的目录，明确告知而不是静默无反应
    setPickError(t(
      `“${picked.name}” is not inside an allowed location. Browse to it below, or paste its absolute path.`,
      `“${picked.name}”不在允许位置内。请在下方浏览，或直接粘贴它的绝对路径。`
    ));
  };

  /** 处理路径输入框回车：POSIX 或 Windows 绝对路径才跳转。 */
  const handleDraftEnter = () => {
    const value = normalizePathInput(draft);
    if (isAbsoluteFilesystemPath(value)) navigate(value);
  };

  /** 在当前浏览目录下创建子目录，成功后刷新列表并选中新目录。 */
  const createFolder = async () => {
    const parent = listing.data?.current;
    const name = newFolderName.trim();
    if (!parent || !name) return;
    setCreateError(null);
    try {
      // 1. 调用后端接口创建目录
      const entry = await api.workspaces.createDirectory(parent, name);
      // 2. 刷新目录列表并选中新目录
      await listing.refetch();
      setSelected(entry.path);
      setCreating(false);
      setNewFolderName("");
    } catch (error) {
      setCreateError(toDisplayError(error, "Failed to create directory", "创建目录失败"));
    }
  };

  /**
   * 执行调用方指定的目录操作，并在弹层内保留可读错误。
   *
   * @returns 无返回值
   */
  const submit = async () => {
    const target = selected || listing.data?.current;
    if (!target) return;
    setSubmitting(true);
    setSubmitError(null);
    try {
      await props.onSelect(target);
    } catch (error) {
      setSubmitError(toDisplayError(error, "Directory action failed", "目录操作失败"));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Modal
      open={props.open}
      title={props.title ?? t("Open server workspace", "打开服务端工作区")}
      description={props.description ?? t("Choose a directory on the server running Sai Web. Server configuration limits the browsing scope.", "选择运行 Sai Web 的服务器上的目录。浏览范围由服务端配置限制。")}
      size="large"
      onClose={props.onClose}
      footer={(
        <>
          <Button onClick={props.onClose}>{t("Cancel", "取消")}</Button>
          <Button variant="primary" onClick={() => void submit()} disabled={submitting || !listing.data}>
            {submitting
              ? props.pendingLabel ?? t("Opening", "正在打开")
              : selected
                ? props.selectedLabel ?? t("Open selected directory", "打开选中目录")
                : props.currentLabel ?? t("Open current directory", "打开当前目录")}
          </Button>
        </>
      )}
    >
      <div className="server-directory-dialog">
        <aside className="directory-roots">
          <span>{t("Allowed location", "允许位置")}</span>
          {listing.data?.roots.map((root) => <button type="button" key={root.path} onClick={() => navigate(root.path)}><HardDrive size={14} /><span><strong>{root.name}</strong><small>{root.path}</small></span></button>)}
          {/* 浏览器目录选择：只交出相对路径，命中允许根之下的同名目录后跳转 */}
          <input
            ref={pickerRef}
            type="file"
            hidden
            /* webkitdirectory 不在 React 的 JSX 属性表里，用 DOM 属性名透传 */
            {...{ webkitdirectory: "", directory: "" }}
            onChange={(event) => {
              const files = Array.from(event.target.files ?? []);
              event.target.value = "";
              if (files.length > 0) void handlePickedDirectory(files);
            }}
          />
          <button type="button" className="directory-pick-local" onClick={() => pickerRef.current?.click()}>
            <FolderSearch size={14} />
            <span>
              <strong>{t("Pick a folder", "选择文件夹")}</strong>
              <small>{t("Locate it under an allowed location", "在允许位置下定位")}</small>
            </span>
          </button>
          {pickError && <p className="directory-pick-error">{pickError}</p>}
        </aside>
        <section className="directory-browser">
          <header>
            <button type="button" onClick={() => listing.data?.parent && navigate(listing.data.parent)} disabled={!listing.data?.parent} aria-label={t("Parent directory", "上级目录")}><ArrowUp size={14} /></button>
            <input
              className="directory-path-input"
              value={draft}
              placeholder={listing.data?.current ?? t("Enter a filter, or an absolute path (for example /home or C:\\Users) and press Enter", "输入过滤词，或输入绝对路径（如 /home 或 C:\\Users）后回车")}
              spellCheck={false}
              onChange={(event) => setDraft(event.target.value)}
              onKeyDown={(event) => { if (event.key === "Enter") handleDraftEnter(); }}
            />
            {isAbsoluteFilesystemPath(draft) && <button type="button" onClick={handleDraftEnter} aria-label={t("Go to entered path", "跳转到输入路径")}><CornerDownLeft size={14} /></button>}
            <button type="button" onClick={() => { setCreating((value) => !value); setCreateError(null); }} disabled={!listing.data} aria-label={t("New folder", "新建文件夹")}><FolderPlus size={14} /></button>
            <button type="button" onClick={() => setShowHidden((value) => !value)} aria-label={showHidden ? t("Hide dot directories", "隐藏点开头目录") : t("Show dot directories", "显示点开头目录")}>
              {showHidden ? <EyeOff size={14} /> : <Eye size={14} />}
            </button>
          </header>
          <div className="directory-current-path"><code>{listing.data?.current ?? "…"}</code></div>
          <div className="directory-list">
            {creating && (
              <div className="directory-create-row">
                <FolderPlus size={16} />
                <input
                  autoFocus
                  value={newFolderName}
                  placeholder={t("New folder name; press Enter to confirm or Escape to cancel", "新文件夹名称，回车确认，Esc 取消")}
                  spellCheck={false}
                  onChange={(event) => setNewFolderName(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") void createFolder();
                    if (event.key === "Escape") { setCreating(false); setNewFolderName(""); setCreateError(null); }
                  }}
                />
              </div>
            )}
            {createError && <div className="pane-error">{createError.message}</div>}
            {submitError && <div className="pane-error">{submitError.message}</div>}
            {entries.map((entry) => (
              <button type="button" className={selected === entry.path ? "selected" : ""} key={entry.path} onDoubleClick={() => navigate(entry.path)} onClick={() => setSelected(entry.path)}>
                <Folder size={16} /><span><strong>{entry.name}</strong><small>{entry.path}</small></span>{entry.git_repository && <span className="directory-git"><GitBranch size={12} />Git</span>}{selected === entry.path && <Check size={14} />}
              </button>
            ))}
            {entries.length === 0 && <div className="directory-empty">{filter ? t(`No directories match “${filter}”`, `没有匹配“${filter}”的目录`) : hiddenCount > 0 ? t(`The current directory contains only ${hiddenCount} hidden directories`, `当前目录只有 ${hiddenCount} 个隐藏目录`) : t("The current directory has no browsable subdirectories", "当前目录没有可浏览的子目录")}</div>}
            {!showHidden && entries.length > 0 && hiddenCount > 0 && !filter && <div className="directory-hidden-hint">{t(`${hiddenCount} dot directories collapsed`, `已折叠 ${hiddenCount} 个点开头目录`)}</div>}
            {listing.error && <div className="pane-error">{listing.error.message}</div>}
          </div>
        </section>
      </div>
    </Modal>
  );
}

/**
 * 按过滤词做大小写不敏感的目录名子串匹配。
 *
 * @param entries 目录条目
 * @param filter 过滤词，空串时不过滤
 * @returns 匹配的目录条目
 */
function filterEntries(entries: DirectoryEntry[], filter: string): DirectoryEntry[] {
  if (!filter) return entries;
  const lowered = filter.toLowerCase();
  return entries.filter((entry) => entry.name.toLowerCase().includes(lowered));
}

/**
 * 过滤隐藏目录并把普通目录排在前面。
 *
 * @param entries 服务端目录条目
 * @param showHidden 是否显示点开头目录
 * @returns 排序后的目录条目
 */
function sortEntries(entries: DirectoryEntry[], showHidden: boolean): DirectoryEntry[] {
  const visible = showHidden ? entries : entries.filter((entry) => !entry.name.startsWith("."));
  return [...visible].sort((left, right) => {
    const leftHidden = left.name.startsWith(".") ? 1 : 0;
    const rightHidden = right.name.startsWith(".") ? 1 : 0;
    if (leftHidden !== rightHidden) return leftHidden - rightHidden;
    return left.name.localeCompare(right.name);
  });
}
