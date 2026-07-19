import { CheckSquare2, Minus, Plus, RotateCcw, Square, SquareCheckBig, SquareX } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import type { RunGitOperation } from "../types";
import type { GitPatchHunk } from "./partial-diff";
import {
  buildSelectedGitPatch,
  parseSelectableGitPatchHunk,
  type GitPatchApplicationDirection,
  type GitPatchSelectionLine
} from "./partial-line-selection";

type SelectablePatchHunkProps = {
  hunk: GitPatchHunk;
  index: number;
  mode: string;
  busy: boolean;
  runOperation: RunGitOperation;
};

/**
 * 渲染可选择增删行的 Git hunk，并提供选中行与整区块操作。
 *
 * @param props hunk 内容、比较模式、忙碌状态和操作回调
 * @returns 可选择行的部分暂存控件
 */
export function SelectablePatchHunk(props: SelectablePatchHunkProps) {
  const { t } = useI18n();
  const parsed = useMemo(() => parseSelectableGitPatchHunk(props.hunk.patch), [props.hunk.patch]);
  const [selectedLineIds, setSelectedLineIds] = useState<Set<number>>(new Set());

  useEffect(() => {
    setSelectedLineIds(new Set());
  }, [props.hunk.patch]);

  /**
   * 切换单个改动行的选择状态。
   *
   * @param lineId hunk 内稳定行编号
   * @returns 无返回值
   */
  const toggleLine = (lineId: number) => {
    setSelectedLineIds((current) => {
      const next = new Set(current);
      if (next.has(lineId)) next.delete(lineId);
      else next.add(lineId);
      return next;
    });
  };

  /**
   * 对当前选择执行暂存、取消暂存或还原。
   *
   * @param action 后端 Git patch 操作
   * @param direction 补丁生成方向
   * @returns 无返回值
   */
  const runSelected = async (
    action: "stage_patch" | "unstage_patch" | "discard_patch",
    direction: GitPatchApplicationDirection
  ) => {
    const patch = buildSelectedGitPatch(props.hunk.patch, selectedLineIds, direction);
    if (!patch) return;
    const destructive = action === "discard_patch";
    const result = await props.runOperation(action, {
      patch,
      ...(destructive ? {
        confirmTitle: t("Discard selected lines?", "丢弃所选行？"),
        confirmDescription: t("The selected working tree lines cannot be recovered.", "所选工作树内容无法恢复。")
      } : {})
    });
    if (result?.ok) setSelectedLineIds(new Set());
  };

  /**
   * 执行当前完整 hunk 的既有操作。
   *
   * @param action 后端 Git patch 操作
   * @returns Git 操作结果
   */
  const runWholeHunk = (action: "stage_patch" | "unstage_patch" | "discard_patch") => {
    const destructive = action === "discard_patch";
    return props.runOperation(action, {
      patch: props.hunk.patch,
      ...(destructive ? {
        confirmTitle: t("Discard selected hunk?", "丢弃所选变更区块？"),
        confirmDescription: t("The selected working tree lines cannot be recovered.", "所选工作树内容无法恢复。")
      } : {})
    });
  };

  return (
    <section className="git-patch-hunk">
      <header>
        <span>{t(`Change ${props.index + 1}`, `变更 ${props.index + 1}`)}</span>
        <div>
          {props.mode === "staged" ? (
            <Button disabled={props.busy} onClick={() => void runWholeHunk("unstage_patch")}>
              <Minus size={12} />{t("Unstage Hunk", "取消暂存此区块")}
            </Button>
          ) : (
            <>
              <Button variant="primary" disabled={props.busy} onClick={() => void runWholeHunk("stage_patch")}>
                <Plus size={12} />{t("Stage Hunk", "暂存此区块")}
              </Button>
              <Button disabled={props.busy} onClick={() => void runWholeHunk("discard_patch")}>
                <RotateCcw size={12} />{t("Discard Hunk", "丢弃此区块")}
              </Button>
            </>
          )}
        </div>
      </header>
      {parsed ? (
        <>
          <div className="git-line-selection-toolbar">
            <span>{t(`${selectedLineIds.size} lines selected`, `已选择 ${selectedLineIds.size} 行`)}</span>
            <Button disabled={props.busy} onClick={() => setSelectedLineIds(new Set(parsed.changedLineIds))}>
              <CheckSquare2 size={12} />{t("Select changes", "选择全部改动")}
            </Button>
            <Button disabled={props.busy || selectedLineIds.size === 0} onClick={() => setSelectedLineIds(new Set())}>
              <SquareX size={12} />{t("Clear", "清除选择")}
            </Button>
            {props.mode === "staged" ? (
              <Button
                variant="primary"
                disabled={props.busy || selectedLineIds.size === 0}
                onClick={() => void runSelected("unstage_patch", "reverse")}
              >
                <Minus size={12} />{t("Unstage Selected", "取消暂存所选行")}
              </Button>
            ) : (
              <>
                <Button
                  variant="primary"
                  disabled={props.busy || selectedLineIds.size === 0}
                  onClick={() => void runSelected("stage_patch", "forward")}
                >
                  <Plus size={12} />{t("Stage Selected", "暂存所选行")}
                </Button>
                <Button
                  disabled={props.busy || selectedLineIds.size === 0}
                  onClick={() => void runSelected("discard_patch", "reverse")}
                >
                  <RotateCcw size={12} />{t("Discard Selected", "丢弃所选行")}
                </Button>
              </>
            )}
          </div>
          <div className="git-selectable-diff" role="list" aria-label={t("Selectable changed lines", "可选择的改动行")}>
            {parsed.lines.map((line) => (
              <SelectablePatchLine
                key={line.id}
                line={line}
                selected={selectedLineIds.has(line.id)}
                onToggle={toggleLine}
              />
            ))}
          </div>
        </>
      ) : (
        <div className="git-line-selection-unavailable">
          {t("Line selection is unavailable for this file operation.", "此文件操作不支持按行选择。")}
        </div>
      )}
    </section>
  );
}

/**
 * 渲染单行差异；改动行使用复选框提供原子选择。
 *
 * @param props 差异行、选择状态和切换回调
 * @returns 单个差异行
 */
function SelectablePatchLine(props: {
  line: GitPatchSelectionLine;
  selected: boolean;
  onToggle: (lineId: number) => void;
}) {
  const selectable = props.line.kind !== "context";
  const marker = props.line.kind === "added" ? "+" : props.line.kind === "removed" ? "-" : " ";
  return (
    <div className={`git-selectable-diff-line ${props.line.kind}${props.selected ? " selected" : ""}`} role="listitem">
      <span className="git-line-selector-shell">
        {selectable && (
          <Button
            className="git-line-selector"
            onClick={() => props.onToggle(props.line.id)}
            aria-pressed={props.selected}
            aria-label={`${props.line.kind} ${props.line.oldLine ?? props.line.newLine ?? ""}`}
          >
            {props.selected ? <SquareCheckBig size={12} /> : <Square size={12} />}
          </Button>
        )}
      </span>
      <span className="git-line-number">{props.line.oldLine ?? ""}</span>
      <span className="git-line-number">{props.line.newLine ?? ""}</span>
      <code><span>{marker}</span>{props.line.text || " "}</code>
    </div>
  );
}
