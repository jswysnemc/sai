import { GitCompareArrows, X } from "lucide-react";
import type { GitDiffResponse } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import type { RunGitOperation } from "../types";
import type { FileComparisonTarget } from "./file-comparison-state";
import { SourceControlDiff } from "./source-control-diff";

type FileComparisonViewProps = {
  target: FileComparisonTarget;
  data?: GitDiffResponse;
  loading: boolean;
  error?: Error | null;
  busy: boolean;
  runOperation: RunGitOperation;
  onClose: () => void;
};

/**
 * 渲染两个工作树文件的比较结果和关闭入口。
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
      <SourceControlDiff
        data={props.data}
        loading={props.loading}
        error={props.error}
        selectedPath={null}
        busy={props.busy}
        runOperation={props.runOperation}
      />
    </div>
  );
}
