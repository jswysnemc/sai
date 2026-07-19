import { CircleStop, Play, SkipForward } from "lucide-react";
import type { GitInProgressOperation } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import type { RunGitOperation } from "../types";

type InProgressOperationBarProps = {
  operation: GitInProgressOperation;
  conflictedCount: number;
  busy: boolean;
  runOperation: RunGitOperation;
};

/**
 * 渲染 merge、rebase、cherry-pick 或 revert 的继续控制条。
 *
 * @param props 当前操作、冲突数量和执行回调
 * @returns 进行中操作控制条
 */
export function InProgressOperationBar(props: InProgressOperationBarProps) {
  const { t } = useI18n();
  const label = operationLabel(props.operation.kind, t);
  return (
    <div className="git-operation-bar">
      <span>
        <strong>{label}</strong>
        <small>{props.conflictedCount > 0 ? t("Resolve conflicts to continue", "解决冲突后继续") : t("Operation is ready to continue", "操作可以继续")}</small>
      </span>
      <div>
        {props.operation.can_continue && (
          <Button
            variant="primary"
            disabled={props.busy || props.conflictedCount > 0}
            onClick={() => void props.runOperation("continue_operation")}
          >
            <Play size={12} />{t("Continue", "继续")}
          </Button>
        )}
        {props.operation.can_skip && (
          <Button disabled={props.busy} onClick={() => void props.runOperation("skip_operation")}>
            <SkipForward size={12} />{t("Skip", "跳过")}
          </Button>
        )}
        {props.operation.can_abort && (
          <Button
            variant="danger"
            disabled={props.busy}
            onClick={() => void props.runOperation("abort_operation", {
              confirmTitle: t(`Abort ${label}?`, `中止${label}？`),
              confirmDescription: t("Git will restore the state from before this operation.", "Git 将恢复到此操作开始前的状态。")
            })}
          >
            <CircleStop size={12} />{t("Abort", "中止")}
          </Button>
        )}
      </div>
    </div>
  );
}

/**
 * 返回进行中操作的本地化名称。
 *
 * @param kind 后端操作类型
 * @param t 双语文本选择函数
 * @returns 操作名称
 */
function operationLabel(kind: string, t: (english: string, chinese: string) => string): string {
  switch (kind) {
    case "merge": return t("Merge in progress", "合并进行中");
    case "rebase": return t("Rebase in progress", "变基进行中");
    case "cherry_pick": return t("Cherry-pick in progress", "拣选进行中");
    case "revert": return t("Revert in progress", "还原进行中");
    default: return t("Git operation in progress", "Git 操作进行中");
  }
}
