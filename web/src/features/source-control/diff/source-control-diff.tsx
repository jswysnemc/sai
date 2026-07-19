import { Minus, Plus, RotateCcw } from "lucide-react";
import type { GitDiffResponse } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { DiffView } from "../../chat/tool-renderers/diff-view";
import { useI18n } from "../../i18n/use-i18n";
import type { RunGitOperation } from "../types";
import { splitGitPatchHunks, type GitPatchHunk } from "./partial-diff";

type SourceControlDiffProps = {
  data?: GitDiffResponse;
  loading: boolean;
  error?: Error | null;
  selectedPath: string | null;
  busy: boolean;
  runOperation: RunGitOperation;
};

/**
 * 渲染 Source Control Diff，并为完整 hunk 提供部分暂存操作。
 *
 * @param props Diff 数据、加载状态和 Git 操作回调
 * @returns Diff 预览或空状态
 */
export function SourceControlDiff(props: SourceControlDiffProps) {
  const { t } = useI18n();
  if (props.loading) {
    return <div className="git-clean diff-clean">{t("Loading diff...", "正在读取差异…")}</div>;
  }
  if (props.error) return <div className="pane-error">{props.error.message}</div>;
  if (!props.data?.patch) {
    return <div className="git-clean diff-clean">{t("No diff to display", "没有可显示的差异")}</div>;
  }

  const supportsPartial = !props.data.truncated && ["staged", "unstaged"].includes(props.data.mode);
  const hunks = supportsPartial ? splitGitPatchHunks(props.data.patch) : [];
  return (
    <div className="git-diff-shell">
      <div className="git-diff-meta">
        {props.data.base_ref} → {props.data.head_ref}
        {props.selectedPath ? ` · ${props.selectedPath}` : ""}
      </div>
      {props.data.stat && <pre className="git-diff-stat">{props.data.stat}</pre>}
      {hunks.length > 0 ? (
        <div className="git-partial-diff">
          {hunks.map((hunk, index) => (
            <PatchHunk
              key={hunk.id}
              hunk={hunk}
              index={index}
              mode={props.data!.mode}
              busy={props.busy}
              runOperation={props.runOperation}
            />
          ))}
        </div>
      ) : (
        <DiffView source={props.data.patch} headerPath={props.selectedPath ?? undefined} />
      )}
      {props.data.truncated && <div className="git-clean">{t("Diff truncated", "差异已截断")}</div>}
    </div>
  );
}

/**
 * 渲染单个可独立应用的 Diff hunk 和索引操作。
 *
 * @param props hunk 内容、模式和操作回调
 * @returns 单个 hunk 控件
 */
function PatchHunk(props: {
  hunk: GitPatchHunk;
  index: number;
  mode: string;
  busy: boolean;
  runOperation: RunGitOperation;
}) {
  const { t } = useI18n();
  return (
    <section className="git-patch-hunk">
      <header>
        <span>{t(`Change ${props.index + 1}`, `变更 ${props.index + 1}`)}</span>
        <div>
          {props.mode === "staged" ? (
            <Button disabled={props.busy} onClick={() => void props.runOperation("unstage_patch", { patch: props.hunk.patch })}>
              <Minus size={12} />{t("Unstage Hunk", "取消暂存此区块")}
            </Button>
          ) : (
            <>
              <Button variant="primary" disabled={props.busy} onClick={() => void props.runOperation("stage_patch", { patch: props.hunk.patch })}>
                <Plus size={12} />{t("Stage Hunk", "暂存此区块")}
              </Button>
              <Button
                disabled={props.busy}
                onClick={() => void props.runOperation("discard_patch", {
                  patch: props.hunk.patch,
                  confirmTitle: t("Discard selected hunk?", "丢弃所选变更区块？"),
                  confirmDescription: t("The selected working tree lines cannot be recovered.", "所选工作树内容无法恢复。")
                })}
              >
                <RotateCcw size={12} />{t("Discard Hunk", "丢弃此区块")}
              </Button>
            </>
          )}
        </div>
      </header>
      <DiffView source={props.hunk.patch} headerPath={props.hunk.path} />
    </section>
  );
}
