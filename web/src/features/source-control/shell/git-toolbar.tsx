import {
  ArrowDownToLine,
  ArrowUpFromLine,
  CloudDownload,
  FolderGit2,
  GitCompare,
  History,
  RefreshCw,
} from "lucide-react";
import type { ComponentType } from "react";
import type { GitBranch, GitRepositoryState } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { GitBranchMenu } from "../../workspace/git-branch-menu";
import { MoreActionsMenu } from "../actions/more-actions-menu";
import { executeGitCommand } from "../commands/git-command-registry";
import type { GitWatchMode } from "../state/use-git-repository-events";
import type { RunGitOperation } from "../types";
import "./git-toolbar.css";

type GitToolbarProps = {
  state: GitRepositoryState;
  branches: GitBranch[];
  branchesLoading: boolean;
  branchMenuOpen: boolean;
  onBranchMenuOpenChange: (open: boolean) => void;
  suggestBranchNames: boolean;
  mode: GitWatchMode;
  onModeChange: (mode: GitWatchMode) => void;
  countBadge: number | null;
  busy: boolean;
  dirtyTotal: number;
  repoRoot: string | null;
  confirmSync: boolean;
  confirmForcePush: boolean;
  onRefresh: () => void;
  runOperation: RunGitOperation;
};

/** 三种视图的图标与文案。 */
const VIEW_TABS: { mode: GitWatchMode; icon: ComponentType<{ size?: number }>; en: string; zh: string }[] = [
  { mode: "changes", icon: GitCompare, en: "Changes", zh: "变更" },
  { mode: "history", icon: History, en: "Graph", zh: "提交图" },
  { mode: "repositories", icon: FolderGit2, en: "Repositories", zh: "仓库" },
];

/**
 * 渲染 Git 面板顶部工具栏。
 *
 * 三组控件按职责分区：左侧分支切换、中部视图切换、右侧同步动作，
 * 避免早先九个按钮平铺时分不出主次。
 *
 * @param props 仓库状态、视图模式与操作回调
 * @returns 工具栏
 */
export function GitToolbar(props: GitToolbarProps) {
  const { t } = useI18n();
  return (
    <header className="git-toolbar">
      <GitBranchMenu
        state={props.state}
        branches={props.branches}
        loading={props.branchesLoading}
        open={props.branchMenuOpen}
        busy={props.busy}
        suggestBranchNames={props.suggestBranchNames}
        onOpenChange={props.onBranchMenuOpenChange}
        onOperation={props.runOperation}
      />

      <nav className="git-toolbar-views" role="tablist" aria-label={t("Git views", "Git 视图")}>
        {VIEW_TABS.map((tab) => {
          const Icon = tab.icon;
          const active = props.mode === tab.mode;
          return (
            <button
              key={tab.mode}
              type="button"
              role="tab"
              aria-selected={active}
              className={active ? "git-toolbar-view active" : "git-toolbar-view"}
              onClick={() => props.onModeChange(tab.mode)}
            >
              <Icon size={13} />
              <span>{t(tab.en, tab.zh)}</span>
              {tab.mode === "changes" && props.countBadge !== null && (
                <span className="git-view-count-badge">{props.countBadge}</span>
              )}
            </button>
          );
        })}
      </nav>

      <div className="git-toolbar-actions">
        <Button
          className="git-toolbar-icon"
          disabled={props.busy}
          onClick={() => void executeGitCommand("git.fetch", props.runOperation)}
          title={t("Fetch remote updates", "获取远端更新")}
          aria-label={t("Fetch remote updates", "获取远端更新")}
        >
          <CloudDownload size={14} />
        </Button>
        <Button
          className="git-toolbar-icon"
          disabled={props.busy}
          onClick={() => void executeGitCommand("git.pull", props.runOperation)}
          title={t("Pull and merge", "拉取并合并")}
          aria-label={t("Pull and merge", "拉取并合并")}
        >
          <ArrowDownToLine size={14} />
        </Button>
        <Button
          className="git-toolbar-icon"
          disabled={props.busy}
          onClick={() => void executeGitCommand("git.push", props.runOperation)}
          title={t("Push", "推送")}
          aria-label={t("Push", "推送")}
        >
          <ArrowUpFromLine size={14} />
        </Button>
        <Button
          className="git-toolbar-icon"
          disabled={props.busy}
          onClick={props.onRefresh}
          title={t("Refresh", "刷新")}
          aria-label={t("Refresh", "刷新")}
        >
          <RefreshCw size={14} />
        </Button>
        <MoreActionsMenu
          busy={props.busy}
          dirtyTotal={props.dirtyTotal}
          repoRoot={props.repoRoot}
          confirmSync={props.confirmSync}
          confirmForcePush={props.confirmForcePush}
          runOperation={props.runOperation}
        />
      </div>
    </header>
  );
}
