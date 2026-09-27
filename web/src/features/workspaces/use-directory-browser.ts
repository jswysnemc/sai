import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type RefObject } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import type { DirectoryEntry, DirectoryListing } from "../../api/contracts";
import { ensureTrailingSlash, lastSegmentOf, normalizeSlashes, stripTrailingSlash } from "./directory-path-input";
import { sortDirectoryEntries } from "./directory-sorting";

/** 高亮下标 -1 表示列表首行的「上级目录」。 */
const PARENT_ROW = -1;

/** 目录浏览器的状态与操作。 */
export type DirectoryBrowser = {
  listing: UseQueryResult<DirectoryListing>;
  pathInput: string;
  filter: string;
  entries: DirectoryEntry[];
  parent: string | null;
  currentPath: string;
  activeRootPath: string;
  highlight: number;
  submitting: boolean;
  submitError: Error | null;
  inputRef: RefObject<HTMLInputElement | null>;
  listRef: RefObject<HTMLDivElement | null>;
  setHighlight: (index: number) => void;
  changePathInput: (value: string) => void;
  changeFilter: (value: string) => void;
  enterDirectory: (path: string) => void;
  goToParent: () => void;
  submit: (target: string) => Promise<void>;
  submitCurrent: () => void;
  handlePathKeyDown: (event: KeyboardEvent<HTMLInputElement>) => void;
  handleFilterKeyDown: (event: KeyboardEvent<HTMLInputElement>) => void;
};

/**
 * 管理服务端目录浏览：路径导航、子目录过滤、键盘高亮与选定提交。
 *
 * 键盘：上下移动、右方向键或 Tab 进入、左方向键返回上级、回车跳转或选定、Esc 关闭。
 *
 * @param open 弹窗是否打开，打开时重置全部状态
 * @param onSelect 选定目录后的回调，抛错时错误留在弹窗内
 * @param onClose 关闭弹窗
 * @returns 目录浏览状态与操作
 */
export function useDirectoryBrowser(open: boolean, onSelect: (path: string) => Promise<void>, onClose: () => void): DirectoryBrowser {
  // 路径输入框是自由文本，browseDir 才是已提交的浏览目录（空串表示服务端默认目录）
  const [pathInput, setPathInput] = useState("");
  const [browseDir, setBrowseDir] = useState("");
  const [filter, setFilter] = useState("");
  const [highlight, setHighlight] = useState(0);
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<Error | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  // 返回上级后待高亮的来源目录名；仅键盘驱动的高亮变化才滚动列表
  const pendingSelectionRef = useRef<string | null>(null);
  const keyboardNavRef = useRef(false);

  const listing = useQuery({
    queryKey: ["workspace-directories", browseDir],
    queryFn: () => api.workspaces.browse(browseDir ? stripTrailingSlash(browseDir) : undefined),
    enabled: open
  });
  const entries = useMemo(() => {
    const sorted = sortDirectoryEntries(listing.data?.entries ?? []);
    const needle = filter.trim().toLowerCase();
    return needle ? sorted.filter((entry) => entry.name.toLowerCase().includes(needle)) : sorted;
  }, [listing.data?.entries, filter]);
  const parent = listing.data?.parent ?? null;
  const currentPath = listing.data?.current ?? "";

  useEffect(() => {
    // 1. 打开时重置；浏览目录留空，等首次 browse 返回默认目录
    if (!open) return;
    setPathInput("");
    setBrowseDir("");
    setFilter("");
    setHighlight(0);
    setSubmitError(null);
    pendingSelectionRef.current = null;
    const timer = window.setTimeout(() => inputRef.current?.focus(), 50);
    return () => window.clearTimeout(timer);
  }, [open]);

  useEffect(() => {
    // 2. 首次加载完成后把导航栏同步为默认目录
    if (!open || browseDir || !listing.data) return;
    const initial = ensureTrailingSlash(listing.data.current);
    setPathInput(initial);
    setBrowseDir(initial);
  }, [open, browseDir, listing.data]);

  useEffect(() => {
    // 3. 返回上级后把高亮定位回来源目录
    if (!pendingSelectionRef.current) return;
    const index = entries.findIndex((entry) => entry.name === pendingSelectionRef.current);
    pendingSelectionRef.current = null;
    setHighlight(index >= 0 ? index : 0);
  }, [entries]);

  useEffect(() => {
    // 4. 键盘导航时让高亮行保持可见，鼠标悬停不滚动
    if (!keyboardNavRef.current) return;
    keyboardNavRef.current = false;
    const container = listRef.current;
    const row = container?.querySelector<HTMLElement>(`[data-row-index="${highlight}"]`);
    if (!container || !row) return;
    const top = row.offsetTop;
    const bottom = top + row.offsetHeight;
    if (top < container.scrollTop) container.scrollTop = top;
    else if (bottom > container.scrollTop + container.clientHeight) container.scrollTop = bottom - container.clientHeight;
  }, [highlight, entries]);

  /**
   * 进入指定目录：提交浏览目录并同步导航栏文本。
   *
   * @param path 目标目录
   * @returns 无返回值
   */
  const enterDirectory = (path: string): void => {
    const target = ensureTrailingSlash(path);
    setBrowseDir(target);
    setPathInput(target);
    setFilter("");
    setHighlight(0);
    setSubmitError(null);
    inputRef.current?.focus();
  };

  const goToParent = (): void => {
    if (!parent) return;
    pendingSelectionRef.current = lastSegmentOf(ensureTrailingSlash(currentPath));
    enterDirectory(parent);
  };

  /**
   * 提交选定目录；失败时错误保留在弹窗内。
   *
   * @param target 目录绝对路径
   * @returns 提交完成后的 Promise
   */
  const submit = async (target: string): Promise<void> => {
    if (!target) return;
    setSubmitting(true);
    setSubmitError(null);
    try {
      await onSelect(target);
      onClose();
    } catch (error) {
      setSubmitError(toDisplayError(error, "Directory action failed", "目录操作失败"));
    } finally {
      setSubmitting(false);
    }
  };

  const submitCurrent = (): void => {
    if (currentPath) void submit(stripTrailingSlash(ensureTrailingSlash(currentPath)));
  };

  const changePathInput = (value: string): void => {
    setPathInput(value);
    setHighlight(0);
    // 1. 以斜杠结尾视为完整目录，立即浏览
    if (normalizeSlashes(value).endsWith("/")) setBrowseDir(ensureTrailingSlash(value));
  };

  const changeFilter = (value: string): void => {
    setFilter(value);
    setHighlight(0);
  };

  /**
   * 循环移动高亮并标记滚动来源为键盘。
   *
   * @param offset 移动步长
   * @returns 无返回值
   */
  const moveHighlight = (offset: number): void => {
    const minIndex = parent ? PARENT_ROW : 0;
    const maxIndex = entries.length - 1;
    keyboardNavRef.current = true;
    setHighlight((value) => {
      const next = value + offset;
      if (next > maxIndex) return minIndex;
      if (next < minIndex) return maxIndex;
      return next;
    });
  };

  const handlePathKeyDown = (event: KeyboardEvent<HTMLInputElement>): void => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      moveHighlight(event.key === "ArrowDown" ? 1 : -1);
    } else if ((event.key === "ArrowRight" || event.key === "Tab") && entries[highlight]) {
      event.preventDefault();
      enterDirectory(entries[highlight].path);
    } else if (event.key === "ArrowLeft") {
      const element = event.currentTarget;
      const atStart = element.selectionStart === 0 && element.selectionEnd === 0;
      if (atStart || pathInput.endsWith("/")) {
        event.preventDefault();
        goToParent();
      }
    } else if (event.key === "Enter") {
      event.preventDefault();
      const typed = normalizeSlashes(pathInput).trim();
      // 1. 路径有改动先跳转，支持 C: 这类盘符写法；否则作用于高亮行或当前目录
      if (typed && ensureTrailingSlash(typed) !== browseDir) enterDirectory(/^[A-Za-z]:$/.test(typed) ? `${typed}/` : typed);
      else if (highlight === PARENT_ROW) goToParent();
      else if (entries[highlight]) void submit(entries[highlight].path);
      else submitCurrent();
    } else if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    }
  };

  const handleFilterKeyDown = (event: KeyboardEvent<HTMLInputElement>): void => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      moveHighlight(event.key === "ArrowDown" ? 1 : -1);
    } else if (event.key === "Enter") {
      event.preventDefault();
      if (highlight === PARENT_ROW) goToParent();
      else if (entries[highlight]) enterDirectory(entries[highlight].path);
    } else if (event.key === "Escape") {
      // 1. Esc 先清空过滤词，已为空时关闭弹窗
      event.preventDefault();
      if (filter) setFilter("");
      else onClose();
    }
  };

  // 按最长前缀判定当前所在的根，避免文件系统根 `/` 恒为活动态
  const currentWithSlash = ensureTrailingSlash(currentPath);
  const activeRootPath = (listing.data?.roots ?? []).reduce((best, root) => {
    const prefix = ensureTrailingSlash(root.path);
    return currentWithSlash.startsWith(prefix) && prefix.length > best.length ? prefix : best;
  }, "");

  return {
    listing, pathInput, filter, entries, parent, currentPath, activeRootPath, highlight, submitting, submitError, inputRef, listRef,
    setHighlight, changePathInput, changeFilter, enterDirectory, goToParent, submit, submitCurrent, handlePathKeyDown, handleFilterKeyDown
  };
}
