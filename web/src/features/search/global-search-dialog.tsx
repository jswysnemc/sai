import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import {
  CalendarClock,
  CheckCircle2,
  FileCode2,
  FolderOpen,
  PanelLeft,
  Search,
  Settings,
  Sparkles,
  SquareTerminal,
  Wrench
} from "lucide-react";
import type { WorkspaceSessions } from "../../api/contracts";
import { Modal } from "../../shared/ui/dialog/modal";
import { useI18n } from "../i18n/use-i18n";
import "./global-search-dialog.css";

export type GlobalSearchAction =
  | "new-session"
  | "open-workspace"
  | "settings"
  | "scheduled-tasks"
  | "toggle-terminal"
  | "open-tasks"
  | "open-subagents"
  | "open-git"
  | "open-files"
  | "toggle-sidebar";

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
};

type GlobalSearchDialogProps = {
  open: boolean;
  workspaces: WorkspaceSessions[];
  onClose: () => void;
  onAction: (action: GlobalSearchAction) => void;
  onOpenSession: (workspaceId: string, sessionId: string) => void;
};

/**
 * 渲染跨工作区的紧凑搜索面板，统一承载操作、会话和文件树入口。
 *
 * @param props 弹层状态、工作区数据和结果操作回调
 * @returns 搜索弹层
 */
export function GlobalSearchDialog({ open, workspaces, onClose, onAction, onOpenSession }: GlobalSearchDialogProps) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<SearchFilter>("all");
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setFilter("all");
    setActiveIndex(0);
  }, [open]);

  const actions = useMemo<SearchResult[]>(() => [
    { id: "new-session", kind: "action", label: t("New task", "新建任务"), shortcut: "Ctrl+N", icon: CheckCircle2, action: "new-session" },
    { id: "open-workspace", kind: "action", label: t("Open workspace", "打开工作区"), shortcut: "Ctrl+O", icon: FolderOpen, action: "open-workspace" },
    { id: "settings", kind: "action", label: t("Settings", "设置"), icon: Settings, action: "settings" },
    { id: "scheduled-tasks", kind: "action", label: t("Scheduled tasks", "定时任务"), icon: CalendarClock, action: "scheduled-tasks" },
    { id: "toggle-terminal", kind: "action", label: t("Toggle terminal", "切换终端"), shortcut: "Ctrl+J", icon: SquareTerminal, action: "toggle-terminal" },
    { id: "open-tasks", kind: "action", label: t("Background tasks", "后台任务"), icon: Wrench, action: "open-tasks" },
    { id: "open-subagents", kind: "action", label: t("Subagents", "子智能体"), icon: Sparkles, action: "open-subagents" },
    { id: "open-git", kind: "action", label: t("Git changes", "Git 变更"), icon: FileCode2, action: "open-git" },
    { id: "open-files", kind: "action", label: t("Open file tree", "打开文件树"), icon: FileCode2, action: "open-files" },
    { id: "toggle-sidebar", kind: "action", label: t("Toggle sidebar", "切换侧栏"), shortcut: "Ctrl+B", icon: PanelLeft, action: "toggle-sidebar" }
  ], [t]);

  const sessions = useMemo<SearchResult[]>(() => workspaces.flatMap((workspace) => workspace.sessions.map((session) => ({
    id: `session:${workspace.workspace_id}:${session.id}`,
    kind: "session" as const,
    label: session.title,
    detail: workspace.workspace_name,
    icon: CheckCircle2,
    workspaceId: workspace.workspace_id,
    sessionId: session.id
  }))), [workspaces]);

  const files = useMemo<SearchResult[]>(() => workspaces.map((workspace) => ({
    id: `files:${workspace.workspace_id}`,
    kind: "file" as const,
    label: t("Browse workspace files", "浏览工作区文件"),
    detail: workspace.workspace_name,
    icon: FileCode2,
    action: "open-files" as const,
    workspaceId: workspace.workspace_id
  })), [t, workspaces]);

  const results = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    const source = filter === "actions" ? actions : filter === "sessions" ? sessions : filter === "files" ? files : [...actions, ...sessions, ...files];
    if (!normalized) return source;
    return source.filter((item) => `${item.label} ${item.detail ?? ""}`.toLowerCase().includes(normalized));
  }, [actions, files, filter, query, sessions]);

  useEffect(() => {
    setActiveIndex((current) => Math.min(current, Math.max(0, results.length - 1)));
  }, [results.length]);

  /** 执行当前高亮结果。 */
  const runResult = (result: SearchResult | undefined) => {
    if (!result) return;
    onClose();
    if (result.kind === "session" && result.workspaceId && result.sessionId) {
      onOpenSession(result.workspaceId, result.sessionId);
      return;
    }
    if (result.action) onAction(result.action);
  };

  /** 处理搜索框键盘导航。 */
  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex((current) => Math.min(current + 1, Math.max(0, results.length - 1)));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex((current) => Math.max(0, current - 1));
    } else if (event.key === "Enter") {
      event.preventDefault();
      runResult(results[activeIndex]);
    }
  };

  return (
    <Modal open={open} title={t("Search", "搜索")} size="small" className="global-search-modal" onClose={onClose} initialFocusRef={inputRef}>
      <div className="global-search-content">
        <label className="global-search-input">
          <Search size={16} aria-hidden="true" />
          <input ref={inputRef} value={query} onChange={(event) => { setQuery(event.target.value); setActiveIndex(0); }} onKeyDown={handleKeyDown} placeholder={t("Search actions, tasks or files", "搜索操作、任务或文件")} aria-label={t("Search actions, tasks or files", "搜索操作、任务或文件")} autoComplete="off" spellCheck={false} />
        </label>
        <div className="global-search-filters" role="tablist" aria-label={t("Search filters", "搜索筛选")}>{([
          ["all", t("All", "全部")],
          ["actions", t("Actions", "操作")],
          ["sessions", t("Tasks", "任务")],
          ["files", t("Files", "文件")]
        ] as const).map(([id, label]) => <button key={id} type="button" role="tab" aria-selected={filter === id} className={filter === id ? "active" : ""} onClick={() => { setFilter(id); setActiveIndex(0); }}>{label}</button>)}</div>
        <div className="global-search-results" role="listbox" aria-label={t("Search results", "搜索结果")}>
          {results.map((result, index) => {
            const Icon = result.icon;
            return <button key={result.id} type="button" role="option" aria-selected={index === activeIndex} className={index === activeIndex ? "active" : ""} onMouseEnter={() => setActiveIndex(index)} onClick={() => runResult(result)}><Icon size={15} aria-hidden="true" /><span className="global-search-result-label">{result.label}<small>{result.detail}</small></span>{result.shortcut && <kbd>{result.shortcut}</kbd>}</button>;
          })}
          {results.length === 0 && <p className="global-search-empty">{t("No matching results", "没有匹配结果")}</p>}
        </div>
      </div>
    </Modal>
  );
}
