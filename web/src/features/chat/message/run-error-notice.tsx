import { CircleAlert, RotateCcw } from "lucide-react";
import type { ReactNode } from "react";
import { ApiError } from "../../../api/api-error";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { ErrorDetailToggle } from "./error-detail-toggle";
import "./run-error-notice.css";

type RunErrorNoticeProps = {
  message: ReactNode;
  detail?: string | null;
  onRetry?: () => void;
};

/**
 * 渲染运行失败摘要、原始详情和可选重试操作。
 *
 * @param props 错误摘要、详情和重试回调
 * @returns 响应式运行错误提示
 */
export function RunErrorNotice({ message, detail, onRetry }: RunErrorNoticeProps) {
  const { t } = useI18n();
  return (
    <div className="run-error" role="alert">
      <div className="run-error-summary">
        <CircleAlert size={14} aria-hidden />
        <span className="run-error-text">{message}</span>
      </div>
      {onRetry && (
        <Button className="run-error-retry" onClick={onRetry}>
          <RotateCcw size={12} />
          <span>{t("Retry", "重试")}</span>
        </Button>
      )}
      {detail && <ErrorDetailToggle detail={detail} />}
    </div>
  );
}

/**
 * 提取适合错误详情区域展示的原始异常信息。
 *
 * @param error 页面捕获的异常
 * @returns 服务端原文或本地异常堆栈
 */
export function errorDetailForDisplay(error: Error): string {
  if (error instanceof ApiError) return error.rawMessage;
  return error.stack?.trim() || error.message;
}
