import { useQuery } from "@tanstack/react-query";
import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { ArrowDown, ArrowUp, Bot, CalendarClock, FileCode2, FolderOpen, GitCompareArrows, MessageSquare, PanelLeft, Search, Settings, SquarePen, SquareTerminal, Wrench } from "lucide-react";
import { api } from "../../api/client";
import type { WorkspaceSessions } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { Modal } from "../../shared/ui/dialog/modal";
import { SegmentedControl } from "../../shared/ui/segmented-control";
import { modKeyLabel } from "../../shared/mod-key";
import { useI18n } from "../i18n/use-i18n";
import { searchFileResults } from "./search-file-results";
import "./global-search-dialog.css";

export type GlobalSearchAction = "new-session" | "open-workspace" | "settings" | "scheduled-tasks" | "toggle-terminal" | "open-tasks" | "open-subagents" | "open-git" | "open-files" | "toggle-sidebar";
type SearchFilter = "all" | "actions" | "sessions" | "files";
type SearchResult = {
  id: string;
  kind: "action" | "session" | "file";
  label: string;
  detail?: string;
  shortcut?: string;
  icon: typeof Search;
  action?: GlobalSearchAction;
  workspaceId?: string;
  sessionId?: string;
  path?: string;
};
type GlobalSearchDialogProps = {
  open: boolean;
  workspaces: WorkspaceSessions[];
  onClose: () => void;
  onAction: (action: GlobalSearchAction) => void;
  onOpenSession: (workspaceId: string, sessionId: string) => void;
  onOpenFile: (path: string) => void;
};

/**
 * 搜索工作台操作、所有项目会话及当前项目文件树中的文件。
 * @param props 弹层状态、项目数据与结果操作回调
 * @returns 支持键盘导航的搜索弹层
 */
export function GlobalSearchDialog({ open, workspaces, onClose, onAction, onOpenSession, onOpenFile }: GlobalSearchDialogProps) {
  const { t } = useI18n();
  const modifier = modKeyLabel();
  const listId = useId();
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<SearchFilter>("all");
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const activeWorkspace = workspaces.find((workspace) => workspace.active);
  const showFiles = filter === "files" || (filter === "all" && query.trim().length > 0);
  const fileTree = useQuery({ queryKey: ["file-tree"], queryFn: () => api.workspace.tree(), enabled: open && showFiles && Boolean(activeWorkspace) });

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setFilter("all");
    setActiveIndex(0);
  }, [open]);

  const actions = useMemo<SearchResult[]>(() => [
    { id: "new-session", kind: "action", label: t("New task", "新建任务"), shortcut: `${modifier}+Shift+O`, icon: SquarePen, action: "new-session" },
    { id: "open-workspace", kind: "action", label: t("Open workspace", "打开工作区"), icon: FolderOpen, action: "open-workspace" },
    { id: "open-files", kind: "action", label: t("Browse files", "浏览文件"), shortcut: `${modifier}+Shift+E`, icon: FileCode2, action: "open-files" },
    { id: "open-git", kind: "action", label: t("Review changes", "审查变更"), shortcut: `${modifier}+Shift+G`, icon: GitCompareArrows, action: "open-git" },
    { id: "toggle-terminal", kind: "action", label: t("Toggle terminal", "切换终端"), shortcut: `${modifier}+J`, icon: SquareTerminal, action: "toggle-terminal" },
    { id: "toggle-sidebar", kind: "action", label: t("Toggle sidebar", "切换侧栏"), shortcut: `${modifier}+B`, icon: PanelLeft, action: "toggle-sidebar" },
    { id: "settings", kind: "action", label: t("Settings", "设置"), icon: Settings, action: "settings" },
    { id: "scheduled-tasks", kind: "action", label: t("Scheduled tasks", "定时任务"), icon: CalendarClock, action: "scheduled-tasks" },
    { id: "open-tasks", kind: "action", label: t("Background tasks", "后台任务"), icon: Wrench, action: "open-tasks" },
    { id: "open-subagents", kind: "action", label: t("Subagents", "子智能体"), icon: Bot, action: "open-subagents" }
  ], [modifier, t]);
  const sessions = useMemo<SearchResult[]>(() => workspaces.flatMap((workspace) => workspace.sessions.map((session) => ({
    id: `session:${workspace.workspace_id}:${session.id}`, kind: "session", label: session.title,
    detail: workspace.workspace_name, icon: MessageSquare, workspaceId: workspace.workspace_id, sessionId: session.id
  }))), [workspaces]);
  const files = useMemo<SearchResult[]>(() => showFiles ? searchFileResults(fileTree.data ?? [], query).map((file) => ({
    id: `file:${file.path}`, kind: "file", label: file.name, detail: file.path, path: file.path, icon: FileCode2
  })) : [], [fileTree.data, query, showFiles]);
  const results = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    const source = filter === "actions" ? actions : filter === "sessions" ? sessions : filter === "files" ? [] : [...actions, ...sessions];
    const matches = source.filter((item) => `${item.label} ${item.detail ?? ""}`.toLocaleLowerCase().includes(normalized));
    return filter === "files" || filter === "all" ? [...matches, ...files] : matches;
  }, [actions, files, filter, query, sessions]);
  const selectedIndex = Math.min(activeIndex, Math.max(0, results.length - 1));

  useEffect(() => {
    if (open) document.getElementById(`${listId}-${selectedIndex}`)?.scrollIntoView({ block: "nearest" });
  }, [listId, open, selectedIndex, results.length]);

  /**
   * 执行高亮结果，并关闭搜索弹层。
   * @param result 已选结果，无结果时不执行操作
   * @returns 无返回值
   */
  const runResult = (result: SearchResult | undefined) => {
    if (!result) return;
    onClose();
    if (result.kind === "session" && result.workspaceId && result.sessionId) onOpenSession(result.workspaceId, result.sessionId);
    else if (result.kind === "file" && result.path) onOpenFile(result.path);
    else if (result.action) onAction(result.action);
  };

  /**
   * 处理搜索框方向键和回车，输入法组合期间保留原始按键行为。
   * @param event 搜索框键盘事件
   * @returns 无返回值
   */
  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.nativeEvent.isComposing) return;
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const direction = event.key === "ArrowDown" ? 1 : -1;
      setActiveIndex(Math.max(0, Math.min(results.length - 1, selectedIndex + direction)));
    } else if (event.key === "Enter") {
      event.preventDefault();
      runResult(results[selectedIndex]);
    }
  };

  return (
    <Modal open={open} title={t("Search", "搜索")} size="small" className="global-search-modal" onClose={onClose} initialFocusRef={inputRef}>
      <div className="global-search-content">
        <label className="global-search-input">
          <Search size={17} aria-hidden />
          <input ref={inputRef} role="combobox" aria-autocomplete="list" aria-expanded="true" aria-controls={listId} aria-activedescendant={results.length ? `${listId}-${selectedIndex}` : undefined} value={query} onChange={(event) => { setQuery(event.target.value); setActiveIndex(0); }} onKeyDown={handleKeyDown} placeholder={t("Search tasks, commands or project files", "搜索任务、命令或项目文件")} aria-label={t("Search tasks, commands or project files", "搜索任务、命令或项目文件")} autoComplete="off" spellCheck={false} />
          <Button variant="ghost" size="small" onClick={onClose} aria-label={t("Close search", "关闭搜索")}>Esc</Button>
        </label>
        <SegmentedControl value={filter} onChange={(value) => { setFilter(value); setActiveIndex(0); }} ariaLabel={t("Search filters", "搜索筛选")} className="global-search-filters" options={[
          { value: "all", label: t("All", "全部") }, { value: "actions", label: t("Commands", "命令") },
          { value: "sessions", label: t("Tasks", "任务") }, { value: "files", label: t("Files", "文件") }
        ]} />
        <div id={listId} className="global-search-results" role="listbox" aria-label={t("Search results", "搜索结果")}>
          {results.map((result, index) => <Button key={result.id} id={`${listId}-${index}`} variant="ghost" role="option" tabIndex={-1} aria-selected={index === selectedIndex} className={index === selectedIndex ? "active" : ""} onMouseEnter={() => setActiveIndex(index)} onClick={() => runResult(result)}>
            <result.icon size={15} aria-hidden /><span className="global-search-result-label">{result.label}<small>{result.detail}</small></span>{result.shortcut && <kbd>{result.shortcut}</kbd>}
          </Button>)}
          {results.length === 0 && <p className="global-search-empty" role="status">{showFiles && fileTree.isLoading ? t("Loading files…", "正在读取文件…") : t("No matching results", "没有匹配结果")}</p>}
        </div>
        {showFiles && fileTree.isError && <p className="global-search-error" role="status">{t("Could not load project files.", "无法读取项目文件。")} <Button variant="ghost" size="small" onClick={() => void fileTree.refetch()}>{t("Retry", "重试")}</Button></p>}
        <footer className="global-search-footer"><span>{showFiles ? t(`Files in ${activeWorkspace?.workspace_name ?? "current project"}`, `文件范围：${activeWorkspace?.workspace_name ?? "当前项目"}`) : t("Commands and tasks across projects", "工作台命令和所有项目任务")}</span><span><kbd><ArrowUp size={12} /><ArrowDown size={12} /></kbd> {t("navigate", "选择")} <kbd>Enter</kbd> {t("open", "打开")}</span></footer>
      </div>
    </Modal>
  );
}
