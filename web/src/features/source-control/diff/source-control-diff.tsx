import { useEffect, useRef, useState } from "react";
import { Columns2, GitCompare, Loader2, Rows3 } from "lucide-react";
import type { GitDiffResponse } from "../../../api/contracts";
import { DiffView, type DiffLayout } from "../../chat/tool-renderers/diff-view";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import type { RunGitOperation } from "../types";
import { GitDiffStat } from "./git-diff-stat";
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
  const [layout, setLayout] = useState<DiffLayout>("side");
  const shellRef = useRef<HTMLDivElement | null>(null);


  if (props.loading) {
    return (
      <div className="git-diff-empty">
        <Loader2 size={20} className="spin" aria-hidden />
        <span>{t("Loading diff...", "正在读取差异…")}</span>
      </div>
    );
  }
  if (props.error) return <div className="pane-error">{props.error.message}</div>;
  if (!props.data?.patch) {
    return (
      <div className="git-diff-empty">
        <GitCompare size={22} aria-hidden />
        <strong>{t("No diff to display", "没有可显示的差异")}</strong>
        <span>{t("Select a file from the changes list to review its diff", "从左侧变更列表选择文件，在这里查看它的差异")}</span>
      </div>
    );
  }

  const supportsPartial = !props.data.truncated && ["staged", "unstaged"].includes(props.data.mode);
  const hunks = supportsPartial ? splitGitPatchHunks(props.data.patch) : [];
  return (
    <div className="git-diff-shell" ref={shellRef}>
      <div className="git-diff-meta">
        <span className="git-diff-refs">
          <code>{props.data.base_ref}</code>
          <span className="git-diff-refs-arrow" aria-hidden>→</span>
          <code>{props.data.head_ref}</code>
          {props.selectedPath ? <span className="git-diff-refs-path">{props.selectedPath}</span> : null}
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
      {props.data.stat ? <GitDiffStat stat={props.data.stat} /> : null}
      {layout === "side" ? (
        /* 并排是审阅模式：直接渲染整块对照视图；部分暂存留在统一视图 */
        <DiffView
          source={props.data.patch}
          headerPath={props.selectedPath ?? undefined}
          onlyPath={props.selectedPath ?? undefined}
          layout="side"
        />
      ) : hunks.length > 0 ? (
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
        <DiffView
          source={props.data.patch}
          headerPath={props.selectedPath ?? undefined}
          onlyPath={props.selectedPath ?? undefined}
          layout="unified"
        />
      )}
      {props.data.truncated && <div className="git-clean">{t("Diff truncated", "差异已截断")}</div>}
    </div>
  );
}
