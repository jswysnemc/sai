import { ArrowUp, Folder, FolderPlus, GitBranch, HardDrive, Loader2, Plus } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import type { DirectoryEntry } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { Modal } from "../../shared/ui/dialog/modal";
import { useI18n } from "../i18n/use-i18n";
import {
  directoryOfInput,
  ensureTrailingSlash,
  filterOfInput,
  lastSegmentOf,
  stripTrailingSlash
} from "./directory-path-input";

type ServerDirectoryDialogProps = {
  open: boolean;
  title?: string;
  description?: string;
  currentLabel?: string;
  pendingLabel?: string;
  onClose: () => void;
  onSelect: (path: string) => Promise<void>;
};

/**
 * 渲染服务端目录浏览和选择对话框。
 *
 * 路径输入框即状态源：以 `/` 结尾的部分决定浏览目录，末段是
 * 子目录过滤词，导航、跳转与搜索共用一个控件。列表行单击进入
 * 目录，高亮行出现行内选择按钮；底部按钮作用于当前目录。
 * 键盘：↑↓ 移动（含上级行）、→/Tab 进入、← 返回上级、Enter 选定、Esc 关闭。
 *
 * @param props 打开状态、文案覆盖、关闭与目录选择回调
 * @returns 服务端目录选择弹层
 */
export function ServerDirectoryDialog(props: ServerDirectoryDialogProps) {
  const { t } = useI18n();
  const [input, setInput] = useState("");
  // 高亮下标：-1 表示「上级目录」行
  const [highlight, setHighlight] = useState(0);
  const [creating, setCreating] = useState(false);
  const [newFolderName, setNewFolderName] = useState("");
  const [createError, setCreateError] = useState<Error | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<Error | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  // 返回上级后待高亮的来源目录名，列表加载完成后定位
  const pendingSelectionRef = useRef<string | null>(null);

  const directory = directoryOfInput(input);
  const filter = filterOfInput(input).toLowerCase();
  const listing = useQuery({
    queryKey: ["workspace-directories", directory],
    queryFn: () => api.workspaces.browse(directory ? stripTrailingSlash(directory) : undefined),
    enabled: props.open
  });
  const entries = useMemo(() => {
    const sorted = sortEntries(listing.data?.entries ?? []);
    if (!filter) return sorted;
    return sorted.filter((entry) => entry.name.toLowerCase().startsWith(filter));
  }, [listing.data?.entries, filter]);
  const parent = listing.data?.parent ?? null;
  const currentPath = listing.data?.current ?? "";

  useEffect(() => {
    // 1. 打开时重置；输入框初始为空，等首次 browse 返回默认目录
    if (!props.open) return;
    setInput("");
    setHighlight(0);
    setCreating(false);
    setNewFolderName("");
    setCreateError(null);
    setSubmitError(null);
    pendingSelectionRef.current = null;
    window.setTimeout(() => inputRef.current?.focus(), 50);
  }, [props.open]);

  useEffect(() => {
    // 2. 首次加载完成后把输入框同步为默认目录
    if (!props.open || input || !listing.data) return;
    setInput(ensureTrailingSlash(listing.data.current));
  }, [props.open, input, listing.data]);

  useEffect(() => {
    // 3. 返回上级后把高亮定位回来源目录
    if (!pendingSelectionRef.current) return;
    const index = entries.findIndex((entry) => entry.name === pendingSelectionRef.current);
    pendingSelectionRef.current = null;
    setHighlight(index >= 0 ? index : 0);
  }, [entries]);

  useEffect(() => {
    // 4. 高亮变化时保持行可见
    const container = listRef.current;
    const row = container?.querySelector<HTMLElement>(`[data-row-index="${highlight}"]`);
    if (container && row) {
      const top = row.offsetTop;
      const bottom = top + row.offsetHeight;
      if (top < container.scrollTop) container.scrollTop = top;
      else if (bottom > container.scrollTop + container.clientHeight) {
        container.scrollTop = bottom - container.clientHeight;
      }
    }
  }, [highlight, entries]);

  /** 进入指定目录：改写输入框即触发浏览。 */
  const enterDirectory = (path: string) => {
    setInput(ensureTrailingSlash(path));
    setHighlight(0);
    setSubmitError(null);
    inputRef.current?.focus();
  };

  /** 返回上级目录并记住来源目录名用于回位高亮。 */
  const goToParent = () => {
    if (!parent) return;
    pendingSelectionRef.current = lastSegmentOf(ensureTrailingSlash(currentPath));
    enterDirectory(parent);
  };

  /**
   * 提交选定目录并在弹层内保留可读错误。
   *
   * @param target 目录绝对路径
   * @returns 无返回值
   */
  const submit = async (target: string) => {
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

  /** 在当前浏览目录下创建子目录，成功后刷新并进入。 */
  const createFolder = async () => {
    const name = newFolderName.trim();
    if (!currentPath || !name) return;
    setCreateError(null);
    try {
      const entry = await api.workspaces.createDirectory(currentPath, name);
      await listing.refetch();
      setCreating(false);
      setNewFolderName("");
      enterDirectory(entry.path);
    } catch (error) {
      setCreateError(toDisplayError(error, "Failed to create directory", "创建目录失败"));
    }
  };

  /**
   * 路径输入框键盘导航。
   *
   * @param event 键盘事件
   * @returns 无返回值
   */
  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    const minIndex = parent ? -1 : 0;
    const maxIndex = entries.length - 1;
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        setHighlight((value) => (value >= maxIndex ? minIndex : value + 1));
        break;
      case "ArrowUp":
        event.preventDefault();
        setHighlight((value) => (value <= minIndex ? maxIndex : value - 1));
        break;
      case "ArrowRight":
      case "Tab":
        if (highlight >= 0 && entries[highlight]) {
          event.preventDefault();
          enterDirectory(entries[highlight].path);
        }
        break;
      case "ArrowLeft": {
        const element = event.currentTarget;
        const atStart = element.selectionStart === 0 && element.selectionEnd === 0;
        if (atStart || input.endsWith("/")) {
          event.preventDefault();
          goToParent();
        }
        break;
      }
      case "Enter":
        event.preventDefault();
        if (highlight === -1) goToParent();
        else if (entries[highlight]) void submit(entries[highlight].path);
        else if (currentPath) void submit(stripTrailingSlash(ensureTrailingSlash(currentPath)));
        break;
      case "Escape":
        event.preventDefault();
        props.onClose();
        break;
      default:
        break;
    }
  };

  return (
    <Modal
      open={props.open}
      title={props.title ?? t("Open server workspace", "打开服务端工作区")}
      description={props.description ?? t("Choose a directory on the server running Sai Web. Server configuration limits the browsing scope.", "选择运行 Sai Web 的服务器上的目录。浏览范围由服务端配置限制。")}
      size="large"
      onClose={props.onClose}
    >
      <div className="server-directory-dialog">
        <div className="directory-input-shell">
          <Folder size={15} aria-hidden />
          <input
            ref={inputRef}
            className="directory-path-input"
            value={input}
            placeholder={t("Type a path; the last segment filters subdirectories", "输入路径，最后一段作为子目录过滤词")}
            spellCheck={false}
            autoComplete="off"
            onChange={(event) => {
              setInput(event.target.value);
              setHighlight(0);
            }}
            onKeyDown={handleKeyDown}
          />
          {listing.isFetching && <Loader2 size={14} className="spin" aria-hidden />}
        </div>
        {(listing.data?.roots.length ?? 0) > 1 && (
          <div className="directory-roots-row">
            {listing.data?.roots.map((root) => (
              <button type="button" key={root.path} onClick={() => enterDirectory(root.path)} title={root.path}>
                <HardDrive size={12} aria-hidden />
                {root.name}
              </button>
            ))}
          </div>
        )}
        <div className="directory-list" ref={listRef}>
          {listing.error && <div className="directory-error">{listing.error.message}</div>}
          {submitError && <div className="directory-error">{submitError.message}</div>}
          {parent && (
            <button
              type="button"
              data-row-index={-1}
              className={highlight === -1 ? "directory-row highlighted" : "directory-row"}
              onClick={goToParent}
              onMouseEnter={() => setHighlight(-1)}
            >
              <ArrowUp size={14} aria-hidden />
              <span className="directory-row-name">{t(".. (parent directory)", "..（上级目录）")}</span>
            </button>
          )}
          {creating && (
            <div className="directory-create-row">
              <FolderPlus size={15} aria-hidden />
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
          {createError && <div className="directory-error">{createError.message}</div>}
          {entries.map((entry, index) => (
            <button
              type="button"
              key={entry.path}
              data-row-index={index}
              className={highlight === index ? "directory-row highlighted" : "directory-row"}
              onClick={() => enterDirectory(entry.path)}
              onMouseEnter={() => setHighlight(index)}
            >
              <Folder size={14} aria-hidden />
              <span className="directory-row-name">{entry.name}</span>
              {entry.git_repository && <span className="directory-git"><GitBranch size={11} aria-hidden />Git</span>}
              {highlight === index && (
                <span
                  role="button"
                  tabIndex={-1}
                  className="directory-row-select"
                  onClick={(event) => {
                    event.stopPropagation();
                    void submit(entry.path);
                  }}
                >
                  <Plus size={11} aria-hidden />
                  {t("Select", "选择")}
                </span>
              )}
            </button>
          ))}
          {!listing.error && entries.length === 0 && !listing.isLoading && (
            <div className="directory-empty">
              {filter
                ? t(`No directories match “${filterOfInput(input)}”`, `没有匹配“${filterOfInput(input)}”的目录`)
                : t("The current directory has no browsable subdirectories", "当前目录没有可浏览的子目录")}
            </div>
          )}
        </div>
        <footer className="directory-footer">
          <button
            type="button"
            className="directory-new-folder"
            onClick={() => { setCreating((value) => !value); setCreateError(null); }}
            disabled={!listing.data}
            aria-label={t("New folder", "新建文件夹")}
          >
            <FolderPlus size={14} aria-hidden />
          </button>
          <code className="directory-footer-path">{currentPath || "…"}</code>
          <Button
            variant="primary"
            onClick={() => void submit(stripTrailingSlash(ensureTrailingSlash(currentPath)))}
            disabled={submitting || !currentPath}
          >
            <Plus size={13} aria-hidden />
            {submitting
              ? props.pendingLabel ?? t("Opening", "正在打开")
              : props.currentLabel ?? t("Open current directory", "打开当前目录")}
          </Button>
        </footer>
      </div>
    </Modal>
  );
}

/**
 * 目录排序：普通目录在前、点开头目录靠后，同组按本地化名称排序。
 *
 * @param entries 服务端目录条目
 * @returns 排序后的目录条目
 */
function sortEntries(entries: DirectoryEntry[]): DirectoryEntry[] {
  return [...entries].sort((left, right) => {
    const leftHidden = left.name.startsWith(".") ? 1 : 0;
    const rightHidden = right.name.startsWith(".") ? 1 : 0;
    if (leftHidden !== rightHidden) return leftHidden - rightHidden;
    return left.name.localeCompare(right.name);
  });
}
