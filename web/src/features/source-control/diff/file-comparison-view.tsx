import { GitCompare, GitCompareArrows, Loader2, X } from "lucide-react";
import type { GitDiffResponse } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { DiffView } from "../../chat/tool-renderers/diff-view";
import { useI18n } from "../../i18n/use-i18n";
import type { FileComparisonTarget } from "./file-comparison-state";
import { GitDiffStat } from "./git-diff-stat";

type FileComparisonViewProps = {
  target: FileComparisonTarget;
  data?: GitDiffResponse;
  loading: boolean;
  error?: Error | null;
  onClose: () => void;
};

/**
 * 渲染两个工作树文件的比较结果和关闭入口。
 *
 * 任意两个文件的比较不属于「未提交变更」审阅流，也没有暂存语义，
 * 因此直接以并排差异呈现，不复用带操作按钮的文件卡片。
 *
 * @param props 比较目标、查询状态和关闭回调
 * @returns 文件比较视图
 */
export function FileComparisonView(props: FileComparisonViewProps) {
  const { t } = useI18n();
  return (
    <div className="git-file-comparison">
      <header className="git-file-comparison-head">
        <span title={`${props.target.basePath} → ${props.target.headPath}`}>
          <GitCompareArrows size={13} />
          {t("File Comparison", "文件比较")}
        </span>
        <Button
          className="git-icon-action"
          onClick={props.onClose}
          title={t("Close comparison", "关闭比较")}
          aria-label={t("Close comparison", "关闭比较")}
        >
          <X size={12} />
        </Button>
      </header>
      <FileComparisonBody data={props.data} loading={props.loading} error={props.error} />
    </div>
  );
}

/**
 * 渲染比较正文：加载、错误、空态与并排差异。
 *
 * @param props 比较查询状态
 * @returns 比较正文
 */
function FileComparisonBody(props: Pick<FileComparisonViewProps, "data" | "loading" | "error">) {
  const { t } = useI18n();
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
        <strong>{t("No differences", "两个文件内容一致")}</strong>
        <span>{t("The compared files have identical content", "所比较的两个文件内容完全相同")}</span>
      </div>
    );
  }
  return (
    <div className="git-diff-shell">
      {props.data.stat ? <GitDiffStat stat={props.data.stat} /> : null}
      <DiffView source={props.data.patch} layout="side" />
      {props.data.truncated && <div className="git-clean">{t("Diff truncated", "差异已截断")}</div>}
    </div>
  );
}
