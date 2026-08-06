import { useMemo, useState } from "react";
import { ChevronDown, ChevronUp, FileDiff, FolderTree, PanelRightOpen, RotateCcw, X } from "lucide-react";
import { api } from "../../../api/client";
import { toDisplayError } from "../../../api/api-error";
import { DiffView } from "../tool-renderers/diff-view";
import { parseJsonRecord, stringField } from "../tool-renderers/tool-data";
import { useI18n } from "../../i18n/use-i18n";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import type { TurnFileChange } from "./collect-turn-file-changes";
import { openWorkspaceDiff } from "../../workspace/workspace-passive-diff";
import "./turn-file-changes.css";

type TurnFileChangesProps = {
  changes: TurnFileChange[];
  /** 本轮工具列表，用于生成 diff 预览 */
  tools?: readonly ToolLike[];
  /** 会话标识；有 turnId 时启用舍弃改动 */
  sessionId?: string | null;
  /** 轮次标识；有值时启用工作树恢复 */
  turnId?: string | null;
  /** 恢复成功后的回调，用于刷新侧栏 Git 状态 */
  onRestored?: () => void;
};

type ToolLike = {
  name: string;
  arguments?: string;
  argumentsPreview?: string;
  output?: string;
  status?: string;
};

/**
 * 展示本轮全部文件改动摘要，支持按单个文件展开 diff、跳转文件树，以及舍弃改动。
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
   * 从 diff 面板跳转到工作区文件树并定位文件。
   *
   * @param path 目标文件路径
   */
  const revealInTree = (path: string) => {
    window.dispatchEvent(
      new CustomEvent("sai:open-file", {
        detail: { path, reveal: true }
      })
    );
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
   * 舍弃单个文件的本轮改动。
   *
   * @param path 文件路径
   * @returns 无返回值
   */
  const discardOne = async (path: string) => {
    if (!sessionId || !turnId) return;
    const accepted = await confirm({
      title: t("Discard changes to this file?", "舍弃该文件的改动？"),
      description: t(
        `Restore “${path}” to the pre-turn worktree snapshot. This cannot be undone.`,
        `将把“${path}”恢复到本轮开始前的工作树快照，操作不可撤销。`
      ),
      confirmLabel: t("Discard file", "舍弃该文件"),
      danger: true
    });
    if (!accepted) return;
    await restorePaths([path]);
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
        <div className="turn-file-changes-summary">
          <span className="turn-file-changes-mark"><FileDiff size={16} aria-hidden /></span>
          <div>
            <strong>{t(`Edited ${changes.length} files`, `已编辑 ${changes.length} 个文件`)}</strong>
            <span className="turn-file-changes-stats"><b>+{added}</b><i>-{removed}</i></span>
          </div>
        </div>
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
                {/* 1. 文件行即唯一标题 2. 展开后右侧放操作，不再叠第二层路径条 */}
                <div className={`turn-file-change-row${expanded ? " active" : ""}`}>
                  <button
                    type="button"
                    className="turn-file-change-main"
                    onClick={() => toggleDiff(change.path)}
                    aria-expanded={expanded}
                    title={actionLabel(change.action, t)}
                  >
                    <span className="turn-file-path">{change.path}</span>
                    <span className="turn-file-changes-stats"><b>+{change.added}</b><i>-{change.removed}</i></span>
                  </button>
                  <div className="turn-file-change-actions">
                    {canDiscard && (
                      <button
                        type="button"
                        className="turn-file-diff-icon-button danger"
                        disabled={busy}
                        onClick={() => void discardOne(change.path)}
                        title={t("Discard this file", "舍弃该文件改动")}
                        aria-label={t("Discard this file", "舍弃该文件改动")}
                      >
                        <RotateCcw size={13} aria-hidden />
                      </button>
                    )}
                    <button
                      type="button"
                      className="turn-file-diff-icon-button"
                      onClick={() => openDiffInSidebar(change.path)}
                      title={t("Open diff in side panel", "在右侧栏打开差异")}
                      aria-label={t("Open diff in side panel", "在右侧栏打开差异")}
                    >
                      <PanelRightOpen size={13} aria-hidden />
                    </button>
                    {expanded && (
                      <>
                        <button
                          type="button"
                          className="turn-file-diff-icon-button"
                          onClick={() => revealInTree(change.path)}
                          title={t("Reveal in file tree", "在文件树中显示")}
                          aria-label={t("Reveal in file tree", "在文件树中显示")}
                        >
                          <FolderTree size={13} aria-hidden />
                        </button>
                        <button
                          type="button"
                          className="turn-file-diff-icon-button"
                          onClick={() => setActivePath(null)}
                          title={t("Close diff", "关闭差异")}
                          aria-label={t("Close diff", "关闭差异")}
                        >
                          <X size={13} aria-hidden />
                        </button>
                      </>
                    )}
                  </div>
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
      <button
        type="button"
        className="turn-file-changes-collapse"
        aria-expanded={!filesCollapsed}
        onClick={() => setFilesCollapsed((current) => !current)}
      >
        <span>{filesCollapsed ? t("Show files", "展开文件") : t("Collapse files", "收起文件")}</span>
        {filesCollapsed ? <ChevronDown size={14} aria-hidden /> : <ChevronUp size={14} aria-hidden />}
      </button>
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
 * 从本轮编辑工具参数/输出组装 Diff 源文本。
 *
 * @param tools 工具列表
 * @param path 可选目标路径过滤
 * @returns Codex / 合成 patch 文本
 */
function buildTurnDiffSource(tools: readonly ToolLike[], path: string | null): string {
  const chunks: string[] = [];
  for (const tool of tools) {
    if (!["edit_file", "write_file", "str_replace"].includes(tool.name)) continue;
    if (tool.status && tool.status !== "completed") continue;
    const argsText = tool.arguments || tool.argumentsPreview || "";
    const args = parseJsonRecord(argsText);
    if (!args) continue;
    if (tool.name === "edit_file") {
      const patch = stringField(args, "patch");
      if (!patch) continue;
      if (path && !patchIncludesPath(patch, path)) continue;
      chunks.push(path ? extractPatchForPath(patch, path) : patch);
      continue;
    }
    const filePath = stringField(args, "path");
    if (!filePath || (path && !pathsMatch(filePath, path))) continue;
    const content = stringField(args, "content");
    const oldString = stringField(args, "old_string");
    const newString = stringField(args, "new_string");
    if (content || oldString || newString) {
      chunks.push(buildSyntheticPatch(filePath, oldString, newString || content, Boolean(content) && !oldString));
    }
  }
  return chunks.filter(Boolean).join("\n");
}

/**
 * 判断两个路径是否指向同一文件。
 *
 * changed_files 常返回绝对路径，而工具参数多为工作区相对路径，
 * 因此在归一化分隔符后按后缀对齐。
 *
 * @param left 路径一
 * @param right 路径二
 * @returns 是否匹配
 */
function pathsMatch(left: string, right: string): boolean {
  const a = normalizePath(left);
  const b = normalizePath(right);
  if (a === b) return true;
  return a.endsWith(`/${b}`) || b.endsWith(`/${a}`);
}

/**
 * 归一化路径分隔符并去掉开头的 ./ 前缀。
 *
 * @param path 原始路径
 * @returns 归一化路径
 */
function normalizePath(path: string): string {
  return path.replaceAll("\\", "/").replace(/^\.\//, "");
}

/**
 * 判断 patch 是否包含指定路径。
 *
 * @param patch Codex patch
 * @param path 文件路径
 * @returns 是否包含
 */
function patchIncludesPath(patch: string, path: string): boolean {
  return patch.split("\n").some((line) =>
    line.startsWith("*** Add File: ") ||
    line.startsWith("*** Delete File: ") ||
    line.startsWith("*** Update File: ")
      ? pathsMatch(line.slice(line.indexOf(": ") + 2).trim(), path)
      : false
  );
}

/**
 * 从多文件 patch 中提取单个路径的片段。
 *
 * @param patch 完整 patch
 * @param path 目标路径
 * @returns 单文件 patch
 */
function extractPatchForPath(patch: string, path: string): string {
  const lines = patch.split("\n");
  const result: string[] = ["*** Begin Patch"];
  let capturing = false;
  for (const line of lines) {
    if (/^\*\*\* (Add|Delete|Update) File: /.test(line)) {
      const filePath = line.slice(line.indexOf(": ") + 2).trim();
      capturing = pathsMatch(filePath, path);
      if (capturing) result.push(line);
      continue;
    }
    if (line.startsWith("*** End Patch")) {
      capturing = false;
      continue;
    }
    if (capturing) result.push(line);
  }
  result.push("*** End Patch");
  return result.length > 2 ? result.join("\n") : "";
}

/**
 * 将 write_file / str_replace 参数转成可预览的简易 patch。
 *
 * @param path 文件路径
 * @param oldText 旧文本
 * @param newText 新文本
 * @param isAdd 是否整文件新增/覆盖展示为 Add
 * @returns Codex 风格 patch 文本
 */
function buildSyntheticPatch(path: string, oldText: string, newText: string, isAdd: boolean): string {
  if (isAdd) {
    const body = newText.split("\n").map((line) => `+${line}`).join("\n");
    return `*** Begin Patch\n*** Add File: ${path}\n${body}\n*** End Patch`;
  }
  const removed = oldText ? oldText.split("\n").map((line) => `-${line}`).join("\n") : "";
  const added = newText.split("\n").map((line) => `+${line}`).join("\n");
  return `*** Begin Patch\n*** Update File: ${path}\n@@\n${removed}${removed && added ? "\n" : ""}${added}\n*** End Patch`;
}
