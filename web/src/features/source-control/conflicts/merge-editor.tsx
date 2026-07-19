import { useQuery } from "@tanstack/react-query";
import { Check, Columns3, RotateCcw } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../../../api/client";
import { Button } from "../../../shared/ui/button/button";
import { TextArea } from "../../../shared/ui/form/text-area";
import { useI18n } from "../../i18n/use-i18n";
import type { RunGitOperation } from "../types";
import { combineConflictBlocks } from "./merge-content";
import "./merge-editor.css";

type MergeEditorProps = {
  path: string;
  repoRoot: string | null;
  busy: boolean;
  runOperation: RunGitOperation;
  onResolved: () => void;
};

/**
 * 渲染 base/ours/theirs 驱动的三栏 Merge Editor。
 *
 * @param props 冲突路径、忙碌状态和解决回调
 * @returns 冲突编辑器
 */
export function MergeEditor(props: MergeEditorProps) {
  const { t } = useI18n();
  const [draft, setDraft] = useState("");
  const initializedTarget = useRef<string | null>(null);
  const conflict = useQuery({
    queryKey: ["git-conflict", props.repoRoot, props.path],
    queryFn: () => api.workspace.gitConflict(props.path, props.repoRoot ?? undefined)
  });

  useEffect(() => {
    const target = `${props.repoRoot ?? ""}\0${props.path}`;
    if (!conflict.data || initializedTarget.current === target) return;
    initializedTarget.current = target;
    setDraft(conflict.data.current);
  }, [conflict.data, props.path, props.repoRoot]);
  const combinedDraft = useMemo(
    () => (conflict.data ? combineConflictBlocks(conflict.data.current) : null),
    [conflict.data]
  );

  /**
   * 写回编辑结果并将冲突文件加入暂存区。
   *
   * @returns 无返回值
   */
  const save = async () => {
    const result = await props.runOperation("resolve_conflict", {
      path: props.path,
      resolution: "content",
      content: draft
    });
    if (result?.ok) props.onResolved();
  };

  /**
   * 直接采用 ours 或 theirs 版本并加入暂存区。
   *
   * @param resolution 待采用版本
   * @returns 无返回值
   */
  const resolveVersion = async (resolution: "ours" | "theirs") => {
    const result = await props.runOperation("resolve_conflict", {
      path: props.path,
      resolution
    });
    if (result?.ok) props.onResolved();
  };

  if (conflict.isLoading) return <div className="git-clean diff-clean">{t("Loading merge editor...", "正在读取合并编辑器…")}</div>;
  if (conflict.error) return <div className="pane-error">{conflict.error.message}</div>;
  if (!conflict.data) return null;

  const operationKind = conflict.data.state.operation?.kind;
  const rebasing = operationKind === "rebase";
  const cherryPicking = operationKind === "cherry-pick";
  const oursLabel = rebasing ? t("Rebased result", "变基结果") : t("Ours", "当前版本");
  const theirsLabel = rebasing
    ? t("Current commit", "当前提交")
    : cherryPicking
      ? t("Picked commit", "摘取提交")
      : t("Theirs", "合入版本");

  return (
    <section className="git-merge-editor">
      <header>
        <span><Columns3 size={14} /><strong>{props.path}</strong></span>
        <div>
          <Button onClick={() => setDraft(conflict.data.current)}><RotateCcw size={12} />{t("Current", "当前内容")}</Button>
          <Button disabled={props.busy} onClick={() => void resolveVersion("ours")}>{rebasing ? t("Accept Rebased", "采用变基结果") : t("Accept Ours", "采用当前版本")}</Button>
          <Button disabled={props.busy} onClick={() => void resolveVersion("theirs")}>{rebasing ? t("Accept Current Commit", "采用当前提交") : t("Accept Theirs", "采用合入版本")}</Button>
          <Button disabled={combinedDraft === null} onClick={() => combinedDraft !== null && setDraft(combinedDraft)}>{t("Accept Both", "采用双方")}</Button>
          <Button variant="primary" disabled={props.busy} onClick={() => void save()}><Check size={12} />{t("Save & Stage", "保存并暂存")}</Button>
        </div>
      </header>
      <div className="git-merge-columns">
        <MergeReference title={oursLabel} content={conflict.data.ours} />
        <label className="git-merge-result">
          <span>{t("Result", "合并结果")}</span>
          <TextArea value={draft} onChange={(event) => setDraft(event.target.value)} spellCheck={false} />
        </label>
        <MergeReference title={theirsLabel} content={conflict.data.theirs} />
      </div>
      {conflict.data.base !== null && (
        <div className="git-merge-base"><span>{t("Base", "共同基线")}</span><pre>{conflict.data.base}</pre></div>
      )}
    </section>
  );
}

/**
 * 渲染 Merge Editor 的只读参考版本。
 *
 * @param props 标题和可选文件内容
 * @returns 只读版本面板
 */
function MergeReference({ title, content }: { title: string; content: string | null }) {
  const { t } = useI18n();
  return (
    <section className="git-merge-reference">
      <span>{title}</span>
      <pre>{content ?? t("File deleted in this version", "此版本已删除文件")}</pre>
    </section>
  );
}
