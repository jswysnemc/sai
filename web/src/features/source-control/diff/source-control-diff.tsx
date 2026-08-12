import { useEffect, useMemo, useRef, useState } from "react";
import { ChevronsDownUp, ChevronsUpDown, Columns2, GitBranch, GitCompare, Loader2, Rows3 } from "lucide-react";
import type { GitDiffResponse, GitRepositoryState, GitStatusEntry } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import type { DiffFile } from "../../chat/tool-renderers/diff/diff-model";
import { parseDiff } from "../../chat/tool-renderers/diff/diff-parser";
import type { DiffLayout } from "../../chat/tool-renderers/diff-view";
import type { RunGitOperation } from "../types";
import { FileDiffCard } from "./file-diff-card";
import "./source-control-diff.css";

type SourceControlDiffProps = {
  data?: GitDiffResponse;
  loading: boolean;
  error?: Error | null;
  /** 当前仓库状态，提供分支名与每个文件的暂存状态 */
  state: GitRepositoryState;
  selectedPath: string | null;
  busy: boolean;
  runOperation: RunGitOperation;
};

/** 选中文件后卡片高亮的持续时间。 */
const HIGHLIGHT_DURATION_MS = 1600;

/**
 * 渲染 Source Control 审阅区：全部变更文件的内联差异卡片流。
 *
 * 顶部吸附总览栏汇总增删行数与分支，正文把每个文件渲染成
 * 独立卡片（吸附文件头 + 统一差异 + 暂存/丢弃操作）；
 * 左侧列表选中文件时滚动定位到对应卡片，而不是切换整个视图。
 *
 * @param props Diff 数据、仓库状态和 Git 操作回调
 * @returns 差异卡片流或空状态
 */
export function SourceControlDiff(props: SourceControlDiffProps) {
  const { t } = useI18n();
  const [layout, setLayout] = useState<DiffLayout>("unified");
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const [highlightedPath, setHighlightedPath] = useState<string | null>(null);
  const cardsRef = useRef(new Map<string, HTMLElement>());
  const lastScrolledRef = useRef<string | null>(null);

  const patch = props.data?.patch ?? "";
  const reviewMode = props.data?.mode === "branch" ? "branch" : "working_tree";
  const files = useMemo(() => {
    const parsed = parseDiff(patch);
    return reviewMode === "working_tree"
      ? [...parsed, ...placeholderFiles(parsed, props.state.entries)]
      : parsed;
  }, [patch, props.state.entries, reviewMode]);
  const entryByPath = useMemo(() => {
    const map = new Map<string, GitStatusEntry>();
    for (const entry of props.state.entries) map.set(entry.path, entry);
    return map;
  }, [props.state.entries]);
  const totals = useMemo(
    () =>
      files.reduce(
        (sum, file) => ({ added: sum.added + file.added, removed: sum.removed + file.removed }),
        { added: 0, removed: 0 }
      ),
    [files]
  );

  useEffect(() => {
    // 左侧选中文件 → 展开并滚动到对应卡片；同一选择在数据刷新时不重复滚动
    const path = props.selectedPath;
    if (!path || lastScrolledRef.current === path) return;
    const element = cardsRef.current.get(path);
    if (!element) return;
    lastScrolledRef.current = path;
    setCollapsed((current) => {
      if (!current.has(path)) return current;
      const next = new Set(current);
      next.delete(path);
      return next;
    });
    element.scrollIntoView({ block: "start", behavior: "smooth" });
    setHighlightedPath(path);
    const timer = window.setTimeout(() => setHighlightedPath(null), HIGHLIGHT_DURATION_MS);
    return () => window.clearTimeout(timer);
  }, [files, props.selectedPath]);

  useEffect(() => {
    // 清空选择后允许再次选中同一文件时重新定位
    if (!props.selectedPath) lastScrolledRef.current = null;
  }, [props.selectedPath]);

  if (props.loading) {
    return (
      <div className="git-diff-empty">
        <Loader2 size={20} className="spin" aria-hidden />
        <span>{t("Loading diff...", "正在读取差异…")}</span>
      </div>
    );
  }
  if (props.error) return <div className="pane-error">{props.error.message}</div>;
  if (!props.data || files.length === 0) {
    return (
      <div className="git-diff-empty">
        <GitCompare size={22} aria-hidden />
        <strong>{t("No changes to review", "没有待审阅的变更")}</strong>
        <span>
          {reviewMode === "branch"
            ? t("This branch has no differences against its baseline", "当前分支相对基线没有差异")
            : t("The working tree is clean", "工作区很干净，没有未提交的更改")}
        </span>
      </div>
    );
  }

  const allCollapsed = files.length > 0 && files.every((file) => collapsed.has(file.path));

  /**
   * 切换单个文件卡片的折叠状态。
   *
   * @param path 文件路径
   * @returns 无返回值
   */
  const toggleCollapse = (path: string) => {
    setCollapsed((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  return (
    <div className="git-diff-shell git-review-stream">
      <header className="git-review-summary">
        <span className="git-review-summary-title">
          {reviewMode === "branch" ? t("Against baseline", "相对基线") : t("Uncommitted", "未提交")}
        </span>
        <span className="git-review-summary-stats">
          {totals.added > 0 && <b>+{totals.added}</b>}
          {totals.removed > 0 && <i>-{totals.removed}</i>}
          {totals.added === 0 && totals.removed === 0 && <em>{t("No line changes", "无行级改动")}</em>}
        </span>
        <span className="git-review-summary-context">
          <span className="git-review-summary-branch" title={props.state.head}>
            <GitBranch size={11} aria-hidden />
            {reviewMode === "branch" ? props.data.base_ref : props.state.head || "HEAD"}
          </span>
          <span className="git-review-summary-count">
            {t(`${files.length} files`, `${files.length} 个文件`)}
          </span>
        </span>
        <span className="git-review-summary-actions">
          <Button
            className="git-review-summary-action"
            onClick={() => setCollapsed(allCollapsed ? new Set() : new Set(files.map((file) => file.path)))}
            title={allCollapsed ? t("Expand all files", "展开全部文件") : t("Collapse all files", "折叠全部文件")}
            aria-label={allCollapsed ? t("Expand all files", "展开全部文件") : t("Collapse all files", "折叠全部文件")}
          >
            {allCollapsed ? <ChevronsUpDown size={13} /> : <ChevronsDownUp size={13} />}
          </Button>
          <span className="git-diff-layout-toggle" role="group" aria-label={t("Diff layout", "差异布局")}>
            <Button
              className={layout === "unified" ? "is-active" : ""}
              onClick={() => setLayout("unified")}
              title={t("Unified view", "统一视图")}
              aria-label={t("Unified view", "统一视图")}
            >
              <Rows3 size={13} />
            </Button>
            <Button
              className={layout === "side" ? "is-active" : ""}
              onClick={() => setLayout("side")}
              title={t("Side by side view", "并排对比")}
              aria-label={t("Side by side view", "并排对比")}
            >
              <Columns2 size={13} />
            </Button>
          </span>
        </span>
      </header>

      <div className="git-review-cards">
        {files.map((file, index) => (
          <FileDiffCard
            key={`${file.path}-${index}`}
            file={file}
            entry={entryByPath.get(file.path)}
            repoRoot={props.state.repo_root}
            reviewMode={reviewMode}
            layout={layout}
            collapsed={collapsed.has(file.path)}
            highlighted={highlightedPath === file.path}
            busy={props.busy}
            truncated={Boolean(props.data?.truncated)}
            onToggleCollapse={() => toggleCollapse(file.path)}
            runOperation={props.runOperation}
            containerRef={(element) => {
              if (element) cardsRef.current.set(file.path, element);
              else cardsRef.current.delete(file.path);
            }}
          />
        ))}
      </div>
      {props.data.truncated && (
        <div className="git-clean">{t("Diff truncated", "差异已截断")}</div>
      )}
    </div>
  );
}

/**
 * 为补丁中缺席的工作区条目补占位卡片。
 *
 * 二进制或超大的未跟踪文件不会出现在后端合成的补丁里，
 * 若不补位，用户会在列表里看到文件、审阅区却完全找不到。
 *
 * @param parsed 已解析出的文件差异
 * @param entries 仓库全部状态条目
 * @returns 无内容的占位文件差异
 */
function placeholderFiles(parsed: DiffFile[], entries: GitStatusEntry[]): DiffFile[] {
  const seen = new Set(parsed.map((file) => file.path));
  return entries
    .filter((entry) => !seen.has(entry.path))
    .map((entry) => ({
      path: entry.path,
      status: entry.untracked || entry.index_status === "A"
        ? "added"
        : entry.worktree_status === "D" || entry.index_status === "D"
          ? "deleted"
          : "modified",
      added: 0,
      removed: 0,
      lines: [],
    }));
}
