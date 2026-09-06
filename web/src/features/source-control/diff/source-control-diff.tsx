import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { GitCompare, Loader2 } from "lucide-react";
import type { GitDiffResponse, GitRepositoryState, GitStatusEntry } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import type { DiffFile } from "../../chat/tool-renderers/diff/diff-model";
import { parseDiff } from "../../chat/tool-renderers/diff/diff-parser";
import { useDiffViewOptions } from "../../chat/tool-renderers/diff/use-diff-view-options";
import type { RunGitOperation } from "../types";
import { isDiffFileCollapsed } from "./diff-review-state";
import { FileDiffCard } from "./file-diff-card";
import { ReviewToolbar } from "./review-toolbar";
import "./source-control-diff.css";

type SourceControlDiffProps = {
  data?: GitDiffResponse;
  loading: boolean;
  error?: Error | null;
  state: GitRepositoryState;
  selectedPath: string | null;
  busy: boolean;
  runOperation: RunGitOperation;
};

/**
 * 组合多文件审阅流，提供筛选、文件导航和按需展开的大文件正文。
 * @param props 补丁、仓库状态、外部文件选择及 Git 操作回调
 * @returns 可响应容器宽度的差异审阅区
 */
export function SourceControlDiff(props: SourceControlDiffProps) {
  const { t } = useI18n();
  const display = useDiffViewOptions();
  const [collapsed, setCollapsed] = useState<ReadonlyMap<string, boolean>>(new Map());
  const [query, setQuery] = useState("");
  const [activePath, setActivePath] = useState<string | null>(props.selectedPath);
  const cardsRef = useRef(new Map<string, HTMLElement>());
  const lastSelectedRef = useRef<string | null>(null);
  const scrollFrameRef = useRef(0);
  const patch = props.data?.patch ?? "";
  const reviewMode = props.data?.mode === "branch" ? "branch" : "working_tree";
  const parsed = useMemo(() => parseDiff(patch), [patch]);
  const parsedPaths = useMemo(() => new Set(parsed.map((file) => file.path)), [parsed]);
  const files = useMemo(() => reviewMode === "working_tree"
    ? [...parsed, ...placeholderFiles(parsed, props.state.entries)] : parsed,
  [parsed, props.state.entries, reviewMode]);
  const entryByPath = useMemo(() => new Map(props.state.entries.map((entry) => [entry.path, entry])), [props.state.entries]);
  const filesByPath = useMemo(() => new Map(files.map((file) => [file.path, file])), [files]);
  const filtered = useMemo(() => {
    const needle = query.trim().replaceAll("\\", "/").toLocaleLowerCase();
    return needle ? files.filter((file) => file.path.toLocaleLowerCase().includes(needle)) : files;
  }, [files, query]);
  const totals = useMemo(() => files.reduce((sum, file) => ({
    added: sum.added + file.added, removed: sum.removed + file.removed,
  }), { added: 0, removed: 0 }), [files]);
  const currentIndex = Math.max(0, filtered.findIndex((file) => file.path === activePath));
  const allCollapsed = filtered.length > 0 && filtered.every((file) => isDiffFileCollapsed(file, collapsed, props.selectedPath));

  /**
   * 展开指定文件后定位正文，取消尚未执行的旧定位请求。
   * @param path 目标文件路径
   * @returns 无返回值
   */
  const revealFile = useCallback((path: string) => {
    setCollapsed((current) => new Map(current).set(path, false));
    setActivePath(path);
    window.cancelAnimationFrame(scrollFrameRef.current);
    scrollFrameRef.current = window.requestAnimationFrame(() => {
      cardsRef.current.get(path)?.scrollIntoView({ block: "start" });
    });
  }, []);

  useEffect(() => {
    const path = props.selectedPath;
    if (!path) { lastSelectedRef.current = null; return; }
    if (lastSelectedRef.current === path || !filesByPath.has(path)) return;
    // 1. 【差异审阅】【文件定位】外部选择先清除筛选，再等待目标卡片挂载
    if (query) { setQuery(""); return; }
    lastSelectedRef.current = path;
    revealFile(path);
  }, [filesByPath, props.selectedPath, query, revealFile]);

  useEffect(() => () => window.cancelAnimationFrame(scrollFrameRef.current), []);

  useEffect(() => {
    const visible = new Set<string>();
    const firstCard = cardsRef.current.values().next().value;
    const root = firstCard?.closest(".diff-scroll");
    if (!root || typeof IntersectionObserver === "undefined") return;
    const toolbar = firstCard?.closest(".git-review-stream")?.querySelector(".git-review-summary");
    let observer: IntersectionObserver | undefined;
    // 2. 【差异审阅】【文件导航】手动滚动时同步当前文件，排除工具栏遮挡的区域
    /**
     * 按工具栏实际高度重建可见区域，兼容响应式换行和字体缩放。
     * @returns 无返回值
     */
    const observeCards = () => {
      observer?.disconnect();
      visible.clear();
      observer = new IntersectionObserver((entries) => {
        for (const entry of entries) {
          const path = (entry.target as HTMLElement).dataset.filePath;
          if (!path) continue;
          if (entry.isIntersecting && entry.intersectionRect.height > 0) visible.add(path);
          else visible.delete(path);
        }
        const firstVisible = filtered.find((file) => visible.has(file.path));
        if (firstVisible) setActivePath(firstVisible.path);
      }, { root, rootMargin: `-${Math.ceil(toolbar?.getBoundingClientRect().height ?? 80)}px 0px 0px 0px`, threshold: 0 });
      for (const card of cardsRef.current.values()) observer.observe(card);
    };
    observeCards();
    const resize = new ResizeObserver(observeCards);
    if (toolbar) resize.observe(toolbar);
    return () => { observer?.disconnect(); resize.disconnect(); };
  }, [filtered]);

  /**
   * 切换文件折叠状态，明确覆盖大文件的默认折叠规则。
   * @param path 文件路径
   * @returns 无返回值
   */
  const toggleCollapse = useCallback((path: string) => {
    const file = filesByPath.get(path);
    if (!file) return;
    setCollapsed((current) => new Map(current).set(path, !isDiffFileCollapsed(file, current, props.selectedPath)));
  }, [filesByPath, props.selectedPath]);

  /**
   * 登记文件卡片，供列表选择和工具栏导航定位。
   * @param path 文件路径
   * @param element 卡片节点；卸载时为空
   * @returns 无返回值
   */
  const registerCard = useCallback((path: string, element: HTMLElement | null) => {
    if (element) cardsRef.current.set(path, element);
    else cardsRef.current.delete(path);
  }, []);

  /**
   * 在当前筛选结果中定位相邻文件。
   * @param direction 前一项为 -1，后一项为 1
   * @returns 无返回值
   */
  const navigate = (direction: -1 | 1) => {
    const file = filtered[currentIndex + direction];
    if (file) revealFile(file.path);
  };

  /**
   * 统一展开或折叠当前筛选结果，保留其他文件的状态。
   * @returns 无返回值
   */
  const toggleAll = () => setCollapsed((current) => {
    const next = new Map(current);
    for (const file of filtered) next.set(file.path, !allCollapsed);
    return next;
  });

  if (props.loading) return <div className="git-diff-empty"><Loader2 size={20} className="spin" aria-hidden />
    <span>{t("Loading diff...", "正在读取差异…")}</span></div>;
  if (props.error) return <div className="pane-error">{props.error.message}</div>;
  if (!props.data || files.length === 0) return <div className="git-diff-empty">
    <GitCompare size={22} aria-hidden /><strong>{t("No changes to review", "没有待审阅的变更")}</strong>
    <span>{reviewMode === "branch" ? t("This branch has no differences against its baseline", "当前分支相对基线没有差异")
      : t("The working tree is clean", "工作区没有未提交的改动")}</span>
  </div>;

  return <div className="git-diff-shell git-review-stream" ref={display.ref}>
    <ReviewToolbar title={reviewMode === "branch" ? t("Against baseline", "相对基线") : t("Uncommitted", "未提交改动")}
      branch={reviewMode === "branch" ? props.data.base_ref : props.state.head || "HEAD"}
      added={totals.added} removed={totals.removed} fileCount={files.length} visibleCount={filtered.length}
      currentIndex={currentIndex} query={query} allCollapsed={allCollapsed} display={display}
      truncated={Boolean(props.data.truncated)}
      onQueryChange={setQuery} onNavigate={navigate} onToggleAll={toggleAll} />
    <div className="git-review-cards">
      {filtered.map((file) => <FileDiffCard key={file.path} file={file} entry={entryByPath.get(file.path)}
        repoRoot={props.state.repo_root} reviewMode={reviewMode} layout={display.layout} wrap={display.wrap}
        collapsed={isDiffFileCollapsed(file, collapsed, props.selectedPath)} highlighted={activePath === file.path}
        busy={props.busy} truncated={Boolean(props.data?.truncated && (!parsedPaths.has(file.path) || parsed.at(-1)?.path === file.path))} onToggleCollapse={toggleCollapse}
        runOperation={props.runOperation} containerRef={registerCard} />)}
      {!filtered.length && <div className="git-diff-empty"><strong>{t("No matching files", "没有匹配的文件")}</strong>
        <Button variant="ghost" size="small" onClick={() => setQuery("")}>{t("Clear filter", "清除筛选")}</Button></div>}
    </div>
    {props.data.truncated && <div className="git-clean">{t("Diff truncated", "差异已截断")}</div>}
  </div>;
}

/**
 * 为补丁中缺席的工作区条目补充文件卡片，保留二进制或未读取文件的入口。
 * @param parsed 已解析出的文件差异
 * @param entries 仓库全部状态条目
 * @returns 补丁中没有出现的文件条目
 */
function placeholderFiles(parsed: DiffFile[], entries: GitStatusEntry[]): DiffFile[] {
  const seen = new Set(parsed.map((file) => file.path));
  return entries.filter((entry) => !seen.has(entry.path)).map((entry) => ({
    path: entry.path,
    status: entry.untracked || entry.index_status === "A" ? "added"
      : entry.worktree_status === "D" || entry.index_status === "D" ? "deleted" : "modified",
    added: 0, removed: 0, lines: [],
  }));
}
