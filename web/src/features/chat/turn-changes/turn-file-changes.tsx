import { useMemo, useState } from "react";
import { ChevronDown, FileDiff, PanelRightOpen, RotateCcw } from "lucide-react";
import { api } from "../../../api/client";
import { toDisplayError } from "../../../api/api-error";
import { DiffView } from "../tool-renderers/diff-view";
import { useI18n } from "../../i18n/use-i18n";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import type { TurnFileChange } from "./collect-turn-file-changes";
import { buildTurnDiffSource, type DiffSourceTool } from "./build-turn-diff-source";
import { openWorkspaceDiff } from "../../workspace/workspace-passive-diff";
import { FileTypeIcon } from "../../../shared/ui/file-icon";
import "./turn-file-changes.css";

type TurnFileChangesProps = {
  changes: TurnFileChange[];
  /** 本轮工具列表，用于生成 diff 预览 */
  tools?: readonly DiffSourceTool[];
  /** 会话标识；有 turnId 时启用舍弃改动 */
  sessionId?: string | null;
  /** 轮次标识；有值时启用工作树恢复 */
  turnId?: string | null;
  /** 恢复成功后的回调，用于刷新侧栏 Git 状态 */
  onRestored?: () => void;
};

/**
 * 展示本轮全部文件改动摘要，支持按单个文件展开 diff、跳转文件树，以及舍弃改动。
 *
 * 摘要行本身就是文件列表的折叠开关，与工具卡的头部语义一致；
 * 新增、删除、重命名在文件行上直接标出，不必悬停才能得知。
 *
 * @param props 汇总后的改动列表与可选工具原始数据
 * @returns 改动组件；无改动时不渲染
 */
export function TurnFileChanges({
  changes,
  tools = [],
  sessionId,
  turnId,
  onRestored
}: TurnFileChangesProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  // 仅允许同时展开一个文件，避免多文件 diff 一股脑铺开
  const [activePath, setActivePath] = useState<string | null>(null);
  const [filesCollapsed, setFilesCollapsed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const canDiscard = Boolean(sessionId && turnId && changes.length > 0);
  const patchSource = useMemo(
    () => (activePath ? buildTurnDiffSource(tools, activePath) : ""),
    [tools, activePath]
  );
  if (changes.length === 0) return null;
  const added = changes.reduce((sum, item) => sum + item.added, 0);
  const removed = changes.reduce((sum, item) => sum + item.removed, 0);
  const activeChange = changes.find((change) => change.path === activePath) ?? null;

  /**
   * 切换指定文件的 diff 展开状态；再次点击同一文件则收起。
   *
   * @param path 目标文件路径
   */
  const toggleDiff = (path: string) => {
    setActivePath((current) => (current === path ? null : path));
  };

  /**
   * 把指定文件差异发送到右侧栏显示。
   *
   * @param path 目标文件路径
   * @returns 无返回值
   */
  const openDiffInSidebar = (path: string) => {
    openWorkspaceDiff({
      path,
      source: buildTurnDiffSource(tools, path),
      title: path.split(/[\\/]/).filter(Boolean).at(-1) ?? path
    });
  };

  /**
   * 打开当前轮次的首个文件差异进行审阅。
   *
   * @returns 无返回值
   */
  const reviewChanges = () => {
    const target = activeChange ?? changes[0];
    if (target) openDiffInSidebar(target.path);
  };

  /**
   * 舍弃本轮全部文件改动。
   *
   * @returns 无返回值
   */
  const discardAll = async () => {
    if (!sessionId || !turnId) return;
    const accepted = await confirm({
      title: t("Discard all changes from this turn?", "舍弃本轮全部改动？"),
      description: t(
        `This restores ${changes.length} file(s) to the pre-turn worktree snapshot. This cannot be undone.`,
        `将把 ${changes.length} 个文件恢复到本轮开始前的工作树快照，操作不可撤销。`
      ),
      confirmLabel: t("Discard all", "舍弃全部"),
      danger: true
    });
    if (!accepted) return;
    await restorePaths([]);
  };

  /**
   * 调用会话 API 恢复工作树。
   *
   * @param paths 目标路径；空数组表示整轮
   * @returns 无返回值
   */
  const restorePaths = async (paths: string[]) => {
    if (!sessionId || !turnId) return;
    setBusy(true);
    setError(null);
    try {
      const result = await api.sessions.restoreWorktree(sessionId, turnId, paths);
      if (!result.restored) {
        setError(t(
          "No restorable worktree snapshot for this turn",
          "本轮没有可恢复的工作树快照"
        ));
        return;
      }
      if (paths.length === 1 && activePath === paths[0]) {
        setActivePath(null);
      }
      onRestored?.();
      window.dispatchEvent(new CustomEvent("sai:workspace-refresh"));
    } catch (err) {
      setError(toDisplayError(err, "Failed to discard changes", "舍弃改动失败").message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="turn-file-changes" aria-label={t("Turn file changes", "本轮文件改动")}>
      <div className="turn-file-changes-head">
        {/* 摘要行即折叠开关：与工具卡「点头部展开详情」同一套语义 */}
        <button
          type="button"
          className="turn-file-changes-summary"
          onClick={() => setFilesCollapsed((current) => !current)}
          aria-expanded={!filesCollapsed}
        >
          <span className="turn-file-changes-mark" aria-hidden><FileDiff size={14} /></span>
          <strong>{t(`Edited ${changes.length} files`, `已编辑 ${changes.length} 个文件`)}</strong>
          <span className="turn-file-changes-stats"><b>+{added}</b><i>-{removed}</i></span>
          <ChevronDown size={14} className={filesCollapsed ? "" : "rotate"} aria-hidden />
        </button>
        <div className="turn-file-changes-head-actions">
          {canDiscard && (
            <button
              type="button"
              className="turn-file-discard-all"
              disabled={busy}
              onClick={() => void discardAll()}
              title={t("Undo all changes this turn", "撤销本轮全部改动")}
            >
              <RotateCcw size={13} aria-hidden />
              <span>{t("Undo", "撤销")}</span>
            </button>
          )}
          <button
            type="button"
            className="turn-file-review"
            onClick={reviewChanges}
            title={t("Review changes", "审阅改动")}
          >
            <PanelRightOpen size={13} aria-hidden />
            <span>{t("Review", "审阅")}</span>
          </button>
        </div>
      </div>
      {error && <div className="turn-file-changes-error" role="alert">{error}</div>}
      {!filesCollapsed && (
        <ul className="turn-file-changes-list">
          {changes.map((change) => {
            const expanded = activePath === change.path;
            return (
              <li key={`${change.tool}:${change.path}`} className={expanded ? "is-expanded" : ""}>
                <div className={`turn-file-change-row${expanded ? " active" : ""}`}>
                  <button
                    type="button"
                    className="turn-file-change-main"
                    onClick={() => toggleDiff(change.path)}
                    aria-expanded={expanded}
                  >
                    <span className="turn-file-path" title={change.path}>
                      <FileTypeIcon name={change.path} size={14} />
                      <span className="turn-file-name">{fileName(change.path)}</span>
                      {fileDirectory(change.path) && <span className="turn-file-directory">{fileDirectory(change.path)}</span>}
                    </span>
                    {/* 非常规修改（新增/删除/重命名）值得直接标出，默认修改不占位 */}
                    {change.action !== "Edited" && (
                      <span className={`turn-file-action is-${change.action.toLowerCase()}`}>
                        {actionLabel(change.action, t)}
                      </span>
                    )}
                    <span className="turn-file-changes-stats">
                      {change.added > 0 && <b>+{change.added}</b>}
                      {change.removed > 0 && <i>-{change.removed}</i>}
                    </span>
                    <ChevronDown size={14} className={expanded ? "rotate" : ""} aria-hidden />
                  </button>
                </div>
                {expanded && activeChange && (
                  <div className="turn-file-diff-panel" role="region" aria-label={t("File diff", "文件差异")}>
                    {patchSource
                      ? <DiffView source={patchSource} headerPath={change.path} hideHeader />
                      : (
                        <div className="turn-file-diff-empty">
                          {t("No patch preview available for this change", "该改动没有可预览的补丁内容")}
                        </div>
                      )}
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

/**
 * 将动作标签本地化。
 *
 * @param action 动作枚举文本
 * @param t 本地化函数
 * @returns 展示文案
 */
function actionLabel(action: string, t: (en: string, zh: string) => string): string {
  if (action === "Added") return t("Added", "新增");
  if (action === "Deleted") return t("Deleted", "删除");
  if (action === "Renamed") return t("Renamed", "重命名");
  return t("Edited", "修改");
}

/**
 * 从路径中提取文件名，避免长绝对路径挤压统计列。
 *
 * @param path 文件路径
 * @returns 文件名
 */
function fileName(path: string): string {
  return path.split(/[\\/]/u).filter(Boolean).at(-1) ?? path;
}

/**
 * 从路径中提取目录部分，作为低对比辅助信息展示。
 *
 * @param path 文件路径
 * @returns 目录路径；没有目录时返回空字符串
 */
function fileDirectory(path: string): string {
  const normalized = path.replaceAll("\\", "/");
  const lastSlash = normalized.lastIndexOf("/");
  return lastSlash > -1 ? normalized.slice(0, lastSlash) : "";
}
