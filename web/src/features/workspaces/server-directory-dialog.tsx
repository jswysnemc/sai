import { ArrowLeft, Folder, FolderPlus, GitBranch, HardDrive, Loader2, Plus, Search } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import type { DirectoryEntry } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { Modal } from "../../shared/ui/dialog/modal";
import { useI18n } from "../i18n/use-i18n";
import {
  ensureTrailingSlash,
  lastSegmentOf,
  normalizeSlashes,
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
 * 路径与搜索分离：上方导航栏是纯目录路径（粘贴/输入完整路径后回车跳转，
 * 输入以 `/` 结尾的路径即时跳转，支持 `/` 与 `C:/` 盘符根）。工具行把
 * 根目录快捷入口与搜索框并为一行：根目录负责跳转，搜索只负责过滤当前
 * 目录的子目录列表。导航栏左侧的上级按钮与列表首行「..（上级目录）」
 * 都可返回上一级。
 * 键盘：↑↓ 移动、→/Tab 进入、← 返回上级、Enter 跳转或选定、Esc 关闭。
 *
 * @param props 打开状态、文案覆盖、关闭与目录选择回调
 * @returns 服务端目录选择弹层
 */
export function ServerDirectoryDialog(props: ServerDirectoryDialogProps) {
  const { t } = useI18n();
  // 路径输入框是自由文本，browseDir 才是已提交的浏览目录（空串 = 服务端默认目录）
  const [pathInput, setPathInput] = useState("");
  const [browseDir, setBrowseDir] = useState("");
  const [filter, setFilter] = useState("");
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
  // 仅键盘驱动的高亮变化才滚动列表；鼠标悬停滚动会把列表拖得到处跑
  const keyboardNavRef = useRef(false);

  const listing = useQuery({
    queryKey: ["workspace-directories", browseDir],
    queryFn: () => api.workspaces.browse(browseDir ? stripTrailingSlash(browseDir) : undefined),
    enabled: props.open
  });
  const entries = useMemo(() => {
    const sorted = sortEntries(listing.data?.entries ?? []);
    const needle = filter.trim().toLowerCase();
    if (!needle) return sorted;
    return sorted.filter((entry) => entry.name.toLowerCase().includes(needle));
  }, [listing.data?.entries, filter]);
  const parent = listing.data?.parent ?? null;
  const currentPath = listing.data?.current ?? "";

  useEffect(() => {
    // 1. 打开时重置；浏览目录留空，等首次 browse 返回默认目录
    if (!props.open) return;
    setPathInput("");
    setBrowseDir("");
    setFilter("");
    setHighlight(0);
    setCreating(false);
    setNewFolderName("");
    setCreateError(null);
    setSubmitError(null);
    pendingSelectionRef.current = null;
    window.setTimeout(() => inputRef.current?.focus(), 50);
  }, [props.open]);

  useEffect(() => {
    // 2. 首次加载完成后把导航栏同步为默认目录
    if (!props.open || browseDir || !listing.data) return;
    const initial = ensureTrailingSlash(listing.data.current);
    setPathInput(initial);
    setBrowseDir(initial);
  }, [props.open, browseDir, listing.data]);

  useEffect(() => {
    // 3. 返回上级后把高亮定位回来源目录
    if (!pendingSelectionRef.current) return;
    const index = entries.findIndex((entry) => entry.name === pendingSelectionRef.current);
    pendingSelectionRef.current = null;
    setHighlight(index >= 0 ? index : 0);
  }, [entries]);

  useEffect(() => {
    // 4. 仅键盘导航时保持高亮行可见（鼠标悬停不滚动列表）
    if (!keyboardNavRef.current) return;
    keyboardNavRef.current = false;
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

  /** 进入指定目录：提交浏览目录并同步导航栏文本。 */
  const enterDirectory = (path: string) => {
    const target = ensureTrailingSlash(path);
    setBrowseDir(target);
    setPathInput(target);
    setFilter("");
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

  /** 把导航栏文本作为完整路径提交浏览；支持 `C:` 这类盘符写法。 */
  const commitPathInput = () => {
    const raw = normalizeSlashes(pathInput).trim();
    if (!raw) return;
    const target = /^[A-Za-z]:$/.test(raw) ? `${raw}/` : ensureTrailingSlash(raw);
    enterDirectory(target);
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

  /** 键盘移动高亮并标记滚动来源为键盘。 */
  const moveHighlight = (offset: number) => {
    const minIndex = parent ? -1 : 0;
    const maxIndex = entries.length - 1;
    keyboardNavRef.current = true;
    setHighlight((value) => {
      const next = value + offset;
      if (next > maxIndex) return minIndex;
      if (next < minIndex) return maxIndex;
      return next;
    });
  };

  /**
   * 导航栏键盘操作。
   *
   * @param event 键盘事件
   * @returns 无返回值
   */
  const handlePathKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        moveHighlight(1);
        break;
      case "ArrowUp":
        event.preventDefault();
        moveHighlight(-1);
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
        if (atStart || pathInput.endsWith("/")) {
          event.preventDefault();
          goToParent();
        }
        break;
      }
      case "Enter": {
        event.preventDefault();
        const typed = normalizeSlashes(pathInput).trim();
        const pending = typed && ensureTrailingSlash(typed) !== browseDir;
        // 路径已变化先跳转；未变化时 Enter 作用于高亮行或当前目录
        if (pending) commitPathInput();
        else if (highlight === -1) goToParent();
        else if (entries[highlight]) void submit(entries[highlight].path);
        else if (currentPath) void submit(stripTrailingSlash(ensureTrailingSlash(currentPath)));
        break;
      }
      case "Escape":
        event.preventDefault();
        props.onClose();
        break;
      default:
        break;
    }
  };

  /**
   * 搜索框键盘操作：方向键与回车共享列表导航，Esc 先清空搜索词。
   *
   * @param event 键盘事件
   * @returns 无返回值
   */
  const handleSearchKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        moveHighlight(1);
        break;
      case "ArrowUp":
        event.preventDefault();
        moveHighlight(-1);
        break;
      case "Enter":
        event.preventDefault();
        if (entries[highlight]) enterDirectory(entries[highlight].path);
        break;
      case "Escape":
        event.preventDefault();
        if (filter) setFilter("");
        else props.onClose();
        break;
      default:
        break;
    }
  };

  const roots = listing.data?.roots ?? [];
  // 加入文件系统根（如 `/`）后按最长前缀判定活动入口，避免 `/` 按钮恒亮
  const currentWithSlash = ensureTrailingSlash(currentPath);
  const activeRootPath = roots.reduce((best, root) => {
    const prefix = ensureTrailingSlash(root.path);
    return currentWithSlash.startsWith(prefix) && prefix.length > best.length ? prefix : best;
  }, "");
  // 服务端的目录读取错误是英文且缺少指引，换成本地化的可执行提示
  const browseErrorMessage = (() => {
    const message = listing.error?.message;
    if (!message) return null;
    if (message.includes("failed to read directory")) {
      return t(
        "This directory cannot be read (the server may lack permission). Edit the path above or go back to another directory.",
        "无法读取该目录（服务端可能没有权限）。请修改上方路径或改到其他目录。"
      );
    }
    return message;
  })();

  return (
    <Modal
      open={props.open}
      title={props.title ?? t("Open server workspace", "打开服务端工作区")}
      description={props.description ?? t("Browse any directory on the server running Sai Web. The shortcuts below jump to your home folder, the server's start directory, filesystem roots, and paths added via SAI_WEB_WORKSPACE_ROOTS.", "可浏览运行 Sai Web 的服务器上的任意目录。下方为快捷入口：用户主目录、服务端启动目录、文件系统根目录，以及环境变量 SAI_WEB_WORKSPACE_ROOTS 添加的路径。")}
      size="large"
      onClose={props.onClose}
    >
      <div className="server-directory-dialog">
        <div className="directory-input-shell">
          <button
            type="button"
            className="directory-up-button"
            onClick={goToParent}
            disabled={!parent}
            aria-label={t("Go to parent directory", "返回上级目录")}
            title={t("Go to parent directory", "返回上级目录")}
          >
            <ArrowLeft size={15} aria-hidden />
          </button>
          <Folder size={15} aria-hidden />
          <input
            ref={inputRef}
            className="directory-path-input"
            value={pathInput}
            placeholder={t("Full path, e.g. /home/you or C:/Users/you — Enter to navigate", "输入完整路径（如 /home/you 或 C:/Users/you），回车跳转")}
            spellCheck={false}
            autoComplete="off"
            onChange={(event) => {
              const value = event.target.value;
              setPathInput(value);
              setHighlight(0);
              // 以斜杠结尾视为完整目录，立即浏览
              if (normalizeSlashes(value).endsWith("/")) {
                setBrowseDir(ensureTrailingSlash(value));
              }
            }}
            onKeyDown={handlePathKeyDown}
          />
          {listing.isFetching && <Loader2 size={14} className="spin" aria-hidden />}
        </div>
        <div className="directory-toolbar">
          {roots.length > 0 && (
            <div className="directory-roots-row">
              {roots.map((root) => (
                <button
                  type="button"
                  key={root.path}
                  className={ensureTrailingSlash(root.path) === activeRootPath ? "active" : ""}
                  onClick={() => enterDirectory(root.path)}
                  title={root.path}
                >
                  <HardDrive size={12} aria-hidden />
                  {root.name}
                </button>
              ))}
            </div>
          )}
          <div className="directory-search-shell">
            <Search size={13} aria-hidden />
            <input
              className="directory-search-input"
              value={filter}
              placeholder={t("Filter subdirectories", "搜索当前目录下的子目录")}
              spellCheck={false}
              autoComplete="off"
              onChange={(event) => {
                setFilter(event.target.value);
                setHighlight(0);
              }}
              onKeyDown={handleSearchKeyDown}
            />
          </div>
        </div>
        <div className="directory-list" ref={listRef}>
          {browseErrorMessage && <div className="directory-error">{browseErrorMessage}</div>}
          {submitError && <div className="directory-error">{submitError.message}</div>}
          {parent && (
            <button
              type="button"
              data-row-index={-1}
              className={highlight === -1 ? "directory-row highlighted" : "directory-row"}
              onClick={goToParent}
              onMouseEnter={() => setHighlight(-1)}
            >
              <ArrowLeft size={14} aria-hidden />
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
                ? t(`No directories match “${filter}”`, `没有匹配“${filter}”的目录`)
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
            title={t("New folder", "新建文件夹")}
          >
            <FolderPlus size={14} aria-hidden />
          </button>
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
