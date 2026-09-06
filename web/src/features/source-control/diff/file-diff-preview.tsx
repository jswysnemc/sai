import { useQuery } from "@tanstack/react-query";
import { Loader2 } from "lucide-react";
import { useEffect, useMemo } from "react";
import { api } from "../../../api/client";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { DiffCodeView } from "../../chat/tool-renderers/diff/diff-code-view";
import type { DiffFile } from "../../chat/tool-renderers/diff/diff-model";
import { parseDiff } from "../../chat/tool-renderers/diff/diff-parser";
import type { DiffLayout } from "../../chat/tool-renderers/diff-view";
import type { GitReviewDiffMode } from "./diff-mode";

export type LoadedFileDiff = { file: DiffFile; truncated: boolean };

type FileDiffPreviewProps = {
  path: string;
  repoRoot: string;
  mode: GitReviewDiffMode;
  layout: DiffLayout;
  wrap?: boolean;
  onLoaded: (result: LoadedFileDiff) => void;
};

/**
 * 按需读取单个文件的差异，避免仓库汇总截断后无法继续审阅；查询随 Git 状态刷新。
 * @param props 文件路径、仓库、比较模式、显示选项及元信息更新回调
 * @returns 单文件差异，或加载、错误、二进制等状态
 */
export function FileDiffPreview(props: FileDiffPreviewProps) {
  const { t } = useI18n();
  const diff = useQuery({
    queryKey: ["git-review-diff", props.repoRoot, props.mode, props.path],
    queryFn: () => api.workspace.gitReviewDiff(props.mode, props.path, props.repoRoot),
  });
  const file = useMemo<DiffFile | null>(() => {
    if (!diff.data) return null;
    const files = parseDiff(diff.data.patch);
    return files.find((item) => item.path === props.path || item.oldPath === props.path) ?? files[0] ?? {
      path: props.path, status: diff.data.binary_files.includes(props.path) ? "binary" : "modified",
      added: 0, removed: 0, lines: [],
    };
  }, [diff.data, props.path]);

  useEffect(() => {
    if (file) props.onLoaded({ file, truncated: Boolean(diff.data?.truncated) });
  }, [file, diff.data?.truncated, props.onLoaded]);

  if (diff.isLoading) return <div className="git-file-card-pending"><Loader2 size={14} className="spin" aria-hidden />
    <span>{t("Loading file diff...", "正在读取文件差异…")}</span></div>;
  if (diff.error) return <div className="git-file-card-load">
    <span>{(diff.error as Error).message}</span>
    <Button variant="ghost" size="small" onClick={() => void diff.refetch()}>{t("Retry", "重试")}</Button>
  </div>;
  if (!file) return null;
  if (file.status === "binary") return <p className="diff-file-note">{t("Binary file not shown", "二进制文件不展示内容")}</p>;
  if (!file.lines.length) return <p className="diff-file-note">{t("No text changes in this file", "此文件没有文本差异")}</p>;
  return <>
    {diff.data?.truncated && <p className="diff-file-note">{t("This file is too large to show in full", "此文件差异过大，当前仅显示部分内容")}</p>}
    <DiffCodeView file={file} language={file.path.split(".").at(-1)} layout={props.layout} wrap={props.wrap} />
  </>;
}
