import { useState } from "react";
import { Columns2, Rows3 } from "lucide-react";
import type { GitDiffResponse } from "../../../api/contracts";
import { DiffView, type DiffLayout } from "../../chat/tool-renderers/diff-view";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import type { RunGitOperation } from "../types";
import { splitGitPatchHunks } from "./partial-diff";
import { SelectablePatchHunk } from "./selectable-patch-hunk";
import "./source-control-diff.css";

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
  const [layout, setLayout] = useState<DiffLayout>("unified");
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
        <span>
          {props.data.base_ref} → {props.data.head_ref}
          {props.selectedPath ? ` · ${props.selectedPath}` : ""}
        </span>
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
      </div>
      {props.data.stat && <pre className="git-diff-stat">{props.data.stat}</pre>}
      {hunks.length > 0 ? (
        <div className="git-partial-diff">
          {hunks.map((hunk, index) => (
            <SelectablePatchHunk
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
        <DiffView source={props.data.patch} headerPath={props.selectedPath ?? undefined} layout={layout} />
      )}
      {props.data.truncated && <div className="git-clean">{t("Diff truncated", "差异已截断")}</div>}
    </div>
  );
}
