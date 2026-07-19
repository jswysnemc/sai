import { Plus } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { GitCommitSummary } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { Modal } from "../../../shared/ui/dialog/modal";
import { useI18n } from "../../i18n/use-i18n";
import type { RunGitOperation } from "../types";
import { createBranchNameSuggestion } from "../branches/branch-name-suggestion";
import { CommitContextMenu } from "./commit-context-menu";
import { CommitGraphRow } from "./commit-graph-row";
import { calculateGitGraphLanes } from "./graph-lanes";
import { calculateGitGraphWindow } from "./graph-window";
import "./commit-graph.css";

const GRAPH_ROW_HEIGHT = 56;

type CommitGraphProps = {
  commits: GitCommitSummary[];
  activeCommit: string | null;
  busy: boolean;
  locale: string;
  canLoadMore: boolean;
  suggestBranchNames: boolean;
  onSelect: (commit: GitCommitSummary) => void;
  onLoadMore: () => void;
  runOperation: RunGitOperation;
};

type ContextMenuState = {
  commit: GitCommitSummary;
  x: number;
  y: number;
};

/**
 * 渲染带引用、同步方向和提交操作菜单的 Source Control Graph。
 *
 * @param props 提交列表、选择状态和 Git 操作回调
 * @returns 提交图列表
 */
export function CommitGraph(props: CommitGraphProps) {
  const { t } = useI18n();
  const viewportRef = useRef<HTMLDivElement>(null);
  const [menu, setMenu] = useState<ContextMenuState | null>(null);
  const [branchCommit, setBranchCommit] = useState<GitCommitSummary | null>(null);
  const [branchName, setBranchName] = useState("");
  const [viewport, setViewport] = useState({ scrollTop: 0, height: GRAPH_ROW_HEIGHT * 10 });
  const windowState = useMemo(
    () => calculateGitGraphWindow(
      props.commits.length,
      GRAPH_ROW_HEIGHT,
      viewport.scrollTop,
      viewport.height
    ),
    [props.commits.length, viewport]
  );
  const graphLayouts = useMemo(() => calculateGitGraphLanes(props.commits), [props.commits]);
  const visibleCommits = props.commits.slice(windowState.start, windowState.end);

  useEffect(() => {
    const element = viewportRef.current;
    if (!element) return;

    /**
     * 同步滚动容器尺寸，保证响应式布局下仍使用正确可见区。
     *
     * @returns 无返回值
     */
    const updateViewport = () => {
      setViewport({ scrollTop: element.scrollTop, height: element.clientHeight });
    };
    updateViewport();
    const observer = new ResizeObserver(updateViewport);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  /**
   * 打开提交右键菜单，并限制菜单起点不超出视口。
   *
   * @param event 鼠标右键事件
   * @param commit 对应提交
   * @returns 无返回值
   */
  const openContextMenu = (event: React.MouseEvent, commit: GitCommitSummary) => {
    event.preventDefault();
    setMenu({
      commit,
      x: Math.max(8, Math.min(event.clientX, window.innerWidth - 17 * 16)),
      y: Math.max(8, Math.min(event.clientY, window.innerHeight - 25 * 16))
    });
  };

  /**
   * 从选定提交创建并切换到新分支。
   *
   * @returns 无返回值
   */
  const createBranch = async () => {
    const name = branchName.trim();
    if (!branchCommit || !name) return;
    const result = await props.runOperation("create_branch", { branch: name, start_point: branchCommit.sha });
    if (!result?.ok) return;
    setBranchCommit(null);
    setBranchName("");
  };

  /**
   * 打开从提交创建分支的弹层并按配置填入建议名称。
   *
   * @param commit 作为新分支起点的提交
   * @returns 无返回值
   */
  const openBranchDialog = (commit: GitCommitSummary) => {
    setBranchCommit(commit);
    setBranchName(props.suggestBranchNames ? createBranchNameSuggestion() : "");
  };

  return (
    <>
      <div
        ref={viewportRef}
        className="git-commit-graph"
        onScroll={(event) => setViewport((current) => ({
          ...current,
          scrollTop: event.currentTarget.scrollTop
        }))}
      >
        {props.commits.length > 0 ? (
          <div className="git-graph-spacer" style={{ height: windowState.totalHeight }}>
            <div className="git-graph-window" style={{ transform: `translateY(${windowState.offsetTop}px)` }}>
              {visibleCommits.map((commit, visibleIndex) => {
                const index = windowState.start + visibleIndex;
                return (
                  <CommitGraphRow
                    key={commit.sha}
                    commit={commit}
                    layout={graphLayouts[index]}
                    active={props.activeCommit === commit.sha}
                    locale={props.locale}
                    onSelect={() => props.onSelect(commit)}
                    onContextMenu={(event) => openContextMenu(event, commit)}
                  />
                );
              })}
            </div>
          </div>
        ) : (
          <div className="git-clean">{t("No commits yet", "暂无提交记录")}</div>
        )}
        {props.canLoadMore && (
          <div className="git-load-more-shell">
            <Button className="git-load-more" onClick={props.onLoadMore}>{t("Load more", "加载更多")}</Button>
          </div>
        )}
      </div>
      {menu && (
        <CommitContextMenu
          {...menu}
          busy={props.busy}
          runOperation={props.runOperation}
          onView={() => props.onSelect(menu.commit)}
          onCreateBranch={() => openBranchDialog(menu.commit)}
          onClose={() => setMenu(null)}
        />
      )}
      <Modal
        open={Boolean(branchCommit)}
        title={t("Create branch from commit", "从提交创建分支")}
        description={branchCommit ? `${branchCommit.short_sha} · ${branchCommit.subject}` : undefined}
        size="small"
        onClose={() => setBranchCommit(null)}
        footer={(
          <>
            <Button onClick={() => setBranchCommit(null)}>{t("Cancel", "取消")}</Button>
            <Button variant="primary" disabled={props.busy || !branchName.trim()} onClick={() => void createBranch()}>
              <Plus size={12} />{t("Create", "创建")}
            </Button>
          </>
        )}
      >
        <label className="git-graph-branch-field">
          <span>{t("Branch name", "分支名称")}</span>
          <input value={branchName} onChange={(event) => setBranchName(event.target.value)} spellCheck={false} />
        </label>
      </Modal>
    </>
  );
}
