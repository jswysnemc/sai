import { useQuery } from "@tanstack/react-query";
import { ChevronDown, ListChecks, Loader2, Minus, Plus, RotateCcw, Trash2 } from "lucide-react";
import { useState } from "react";
import { api } from "../../../api/client";
import type { GitStatusEntry } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { DiffIdeaView } from "../../chat/tool-renderers/diff-idea-view";
import { DiffUnifiedView } from "../../chat/tool-renderers/diff-unified-view";
import type { DiffFile } from "../../chat/tool-renderers/diff/diff-model";
import type { DiffLayout } from "../../chat/tool-renderers/diff-view";
import { ToolFileReference } from "../../chat/tool-renderers/tool-file-reference";
import type { RunGitOperation } from "../types";
import type { GitReviewDiffMode } from "./diff-mode";
import { splitGitPatchHunks } from "./partial-diff";
import { SelectablePatchHunk } from "./selectable-patch-hunk";

type FileDiffCardProps = {
  file: DiffFile;
  /** 工作区实时状态；分支视图或已提交文件可能没有对应条目 */
  entry?: GitStatusEntry;
  repoRoot: string;
  reviewMode: GitReviewDiffMode;
  layout: DiffLayout;
  collapsed: boolean;
  /** 左侧列表选中的文件，滚动定位后短暂高亮 */
  highlighted: boolean;
  busy: boolean;
  truncated: boolean;
  onToggleCollapse: () => void;
  runOperation: RunGitOperation;
  /** 卡片根元素回调，供父级做滚动定位 */
  containerRef?: (element: HTMLElement | null) => void;
};

/**
 * 渲染单个文件的差异卡片：吸附文件头 + 内联统一差异。
 *
 * 文件头汇集状态圆点、路径、增删统计与暂存/丢弃操作；
 * 「行级操作」按需拉取该文件的暂存区补丁，切换成可按行暂存的视图。
 *
 * @param props 解析后的文件差异、实时状态与 Git 操作回调
 * @returns 文件差异卡片
 */
export function FileDiffCard(props: FileDiffCardProps) {
  const { t } = useI18n();
  const [lineOps, setLineOps] = useState(false);

  const { file, entry } = props;
  const tone = fileTone(file, entry);
  const badge = fileBadge(entry, t);
  const deleted = tone === "deleted";
  const interactive = props.reviewMode === "working_tree" && Boolean(entry);
  const hasWorktreeChanges = Boolean(entry && (entry.worktree_status !== "." || entry.untracked));
  const canStage = interactive && Boolean(entry && (hasWorktreeChanges || entry.conflicted));
  const canUnstage = interactive && Boolean(entry?.staged) && !entry?.conflicted;
  const canDiscard = interactive && hasWorktreeChanges && !entry?.conflicted;
  const lineOpsMode: "staged" | "unstaged" = hasWorktreeChanges ? "unstaged" : "staged";
  const canLineOps =
    interactive && !props.truncated && !entry?.conflicted && file.status !== "binary" && file.lines.length > 0;

  /**
   * 经确认后丢弃该文件的工作区改动。
   *
   * @returns 无返回值
   */
  const discard = () => {
    if (!entry) return;
    void props.runOperation("discard", {
      path: entry.path,
      old_path: entry.old_path ?? undefined,
      confirmTitle: entry.untracked
        ? t("Delete untracked file", "删除未跟踪文件")
        : t("Discard working tree changes", "撤销工作区修改"),
      confirmDescription: entry.untracked
        ? t(`Permanently delete “${entry.path}”.`, `将永久删除“${entry.path}”。`)
        : t(`Restore ${entry.path}. Unsaved changes cannot be recovered.`, `将恢复 ${entry.path}，未保存修改无法恢复。`),
    });
  };

  const directory = fileDirectory(file.path);
  return (
    <section
      ref={props.containerRef}
      className={`git-file-card${props.collapsed ? " is-collapsed" : ""}${props.highlighted ? " is-highlighted" : ""}`}
    >
      <header
        className="git-file-card-head"
        role="button"
        tabIndex={0}
        aria-expanded={!props.collapsed}
        title={file.path}
        onClick={props.onToggleCollapse}
        onKeyDown={(event) => {
          if (event.key !== "Enter" && event.key !== " ") return;
          event.preventDefault();
          props.onToggleCollapse();
        }}
      >
        <span className={`git-file-card-dot tone-${tone}`} aria-hidden />
        <span className={`git-file-card-path${deleted ? " is-deleted" : ""}`}>
          {directory && <span className="git-file-card-dir">{directory}/</span>}
          <ToolFileReference
            path={file.path}
            label={fileName(file.path)}
            icon={false}
            className="git-file-card-name"
          />
        </span>
        {badge && <span className={`git-file-card-badge tone-${tone}`}>{badge}</span>}
        <span className="git-file-card-stats">
          {file.added > 0 && <b>+{file.added}</b>}
          {file.removed > 0 && <i>-{file.removed}</i>}
        </span>
        {/* 操作按钮不参与折叠切换；分支审阅等只读场景不渲染空容器 */}
        {(canLineOps || canUnstage || canStage || canDiscard) && (
          <span className="git-file-card-actions" onClick={(event) => event.stopPropagation()}>
            {canLineOps && (
              <Button
                className={`git-file-card-action${lineOps ? " is-active" : ""}`}
                disabled={props.busy}
                onClick={() => setLineOps((value) => !value)}
                title={lineOps ? t("Back to inline diff", "返回内联差异") : t("Stage lines or hunks", "按行或区块暂存")}
                aria-pressed={lineOps}
              >
                <ListChecks size={13} />
              </Button>
            )}
            {canUnstage && (
              <Button
                className="git-file-card-action"
                disabled={props.busy}
                onClick={() => void props.runOperation("unstage", { path: file.path })}
                title={t("Unstage", "取消暂存")}
              >
                <Minus size={13} />
              </Button>
            )}
            {canStage && (
              <Button
                className="git-file-card-action"
                disabled={props.busy}
                onClick={() => void props.runOperation("stage", { path: file.path })}
                title={entry?.conflicted ? t("Mark as resolved", "标记为已解决") : t("Stage", "暂存")}
              >
                <Plus size={13} />
              </Button>
            )}
            {canDiscard && (
              <Button
                className="git-file-card-action is-danger"
                disabled={props.busy}
                onClick={discard}
                title={entry?.untracked ? t("Delete untracked file", "删除未跟踪文件") : t("Discard changes", "撤销修改")}
              >
                {entry?.untracked ? <Trash2 size={13} /> : <RotateCcw size={13} />}
              </Button>
            )}
          </span>
        )}
        <ChevronDown size={14} className={`git-file-card-chevron${props.collapsed ? "" : " open"}`} aria-hidden />
      </header>

      {!props.collapsed && (
        <div className="git-file-card-body">
          {file.oldPath && (
            <p className="diff-file-note">{t(`Renamed from ${file.oldPath}`, `由 ${file.oldPath} 重命名`)}</p>
          )}
          {file.status === "binary" && (
            <p className="diff-file-note">{t("Binary file not shown", "二进制文件不展示内容")}</p>
          )}
          {file.status !== "binary" && file.lines.length === 0 && (
            <p className="diff-file-note">
              {t("No previewable text changes for this file", "此文件没有可预览的文本改动")}
            </p>
          )}
          {lineOps && canLineOps ? (
            <FileLineOps
              path={file.path}
              repoRoot={props.repoRoot}
              mode={lineOpsMode}
              busy={props.busy}
              runOperation={props.runOperation}
            />
          ) : (
            file.lines.length > 0 && (
              /* structured-diff 提供代码字体与配色；紧凑模式去掉自带底色，由卡片承担 */
              <div className="structured-diff is-compact">
                {props.layout === "side" ? (
                  <DiffIdeaView file={file} language={languageOfPath(file.path)} />
                ) : (
                  <DiffUnifiedView file={file} language={languageOfPath(file.path)} />
                )}
              </div>
            )
          )}
        </div>
      )}
    </section>
  );
}

type FileLineOpsProps = {
  path: string;
  repoRoot: string;
  mode: "staged" | "unstaged";
  busy: boolean;
  runOperation: RunGitOperation;
};

/**
 * 行级操作视图：按需拉取该文件的暂存区补丁并拆成可独立暂存的 hunk。
 *
 * 卡片流展示的是 HEAD 到工作树的合并差异，无法直接交给 git apply；
 * 这里以暂存区为基准重新取单文件补丁，保证按行暂存/丢弃可精确应用。
 * 查询键沿用 git-review-diff 前缀，任何暂存操作后随状态作用域一起失效。
 *
 * @param props 文件路径、仓库根目录与比较基准
 * @returns 可按行暂存的 hunk 列表
 */
function FileLineOps(props: FileLineOpsProps) {
  const { t } = useI18n();
  const diff = useQuery({
    queryKey: ["git-review-diff", props.repoRoot, props.mode, props.path],
    queryFn: () => api.workspace.gitReviewDiff(props.mode, props.path, props.repoRoot),
  });

  if (diff.isLoading) {
    return (
      <div className="git-file-card-pending">
        <Loader2 size={14} className="spin" aria-hidden />
        <span>{t("Loading line-level diff...", "正在读取行级差异…")}</span>
      </div>
    );
  }
  if (diff.error) return <p className="diff-file-note">{(diff.error as Error).message}</p>;

  const hunks = splitGitPatchHunks(diff.data?.patch ?? "");
  if (hunks.length === 0) {
    return (
      <p className="diff-file-note">
        {props.mode === "staged"
          ? t("Nothing staged for this file", "此文件没有已暂存的内容")
          : t("No unstaged lines for this file", "此文件没有未暂存的改动行")}
      </p>
    );
  }
  return (
    <div className="git-partial-diff">
      {hunks.map((hunk, index) => (
        <SelectablePatchHunk
          key={hunk.id}
          hunk={hunk}
          index={index}
          mode={props.mode}
          busy={props.busy}
          runOperation={props.runOperation}
        />
      ))}
    </div>
  );
}

/**
 * 根据实时状态与补丁内容推断状态圆点色调。
 *
 * @param file 解析后的文件差异
 * @param entry 工作区实时状态
 * @returns CSS 色调名称
 */
export function fileTone(file: DiffFile, entry?: GitStatusEntry): string {
  if (entry?.conflicted) return "conflict";
  if (entry) {
    if (entry.worktree_status === "D" || entry.index_status === "D") return "deleted";
    if (entry.untracked || entry.index_status === "A") return "added";
    return "modified";
  }
  if (file.status === "deleted") return "deleted";
  if (file.status === "added") return "added";
  return "modified";
}

/**
 * 返回文件头的暂存状态徽标文案。
 *
 * 卡片流以工作树为基准合并展示，暂存进度不再由分区体现，
 * 因此在文件头补一枚徽标标注“已暂存/部分暂存”。
 *
 * @param entry 工作区实时状态
 * @param t 本地化函数
 * @returns 徽标文案；普通修改返回 null
 */
function fileBadge(entry: GitStatusEntry | undefined, t: (en: string, zh: string) => string): string | null {
  if (!entry) return null;
  if (entry.conflicted) return t("Conflict", "冲突");
  if (entry.untracked) return t("Untracked", "未跟踪");
  if (entry.staged && entry.worktree_status !== ".") return t("Partially staged", "部分暂存");
  if (entry.staged) return t("Staged", "已暂存");
  return null;
}

/**
 * 从差异路径提取文件名；未跟踪目录条目以尾斜杠结尾，保留斜杠以示目录。
 *
 * @param path 文件路径
 * @returns 文件名或目录名
 */
function fileName(path: string): string {
  const name = path.split(/[\\/]/u).filter(Boolean).at(-1) ?? path;
  return /[\\/]$/u.test(path) ? `${name}/` : name;
}

/**
 * 从差异路径提取目录部分。
 *
 * @param path 文件路径
 * @returns 目录路径；没有目录时返回空
 */
function fileDirectory(path: string): string {
  const normalized = path.replaceAll("\\", "/").replace(/\/+$/u, "");
  const slash = normalized.lastIndexOf("/");
  return slash > -1 ? normalized.slice(0, slash) : "";
}

/**
 * 从文件路径推断代码着色语言。
 *
 * @param path 文件路径
 * @returns 扩展名语言标识，无扩展名时为 undefined
 */
function languageOfPath(path: string): string | undefined {
  const name = path.split("/").pop() ?? "";
  return name.includes(".") ? name.split(".").pop() : undefined;
}
