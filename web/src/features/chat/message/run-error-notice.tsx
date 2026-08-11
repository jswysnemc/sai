import { CircleAlert, RotateCcw } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
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
 * 单层卡片：标题行左摘要、右操作；详情展开时只加顶部分隔，不再套内层灰框。
 *
 * @param props 错误摘要、详情和重试回调
 * @returns 响应式运行错误提示
 */
export function RunErrorNotice({ message, detail, onRetry }: RunErrorNoticeProps) {
  const { t } = useI18n();
  const [detailOpen, setDetailOpen] = useState(false);
  const text = typeof message === "string" ? message : "";
  const disconnect =
    text.includes("连接中断")
    || text.includes("Connection interrupted")
    || text.includes("运行事件流已断开")
    || text.includes("event stream disconnected");
  const interrupted =
    text.includes("运行已中断")
    || text.includes("The run was interrupted")
    || text.includes("响应已中断")
    || text.includes("The response was interrupted");
  const detailText = detail?.trim() || "";
  // 中断类提示直接露出说明，避免只剩标题、详情还要二次点击
  const showInlineDetail = interrupted && Boolean(detailText);
  const showDetailToggle = Boolean(detailText) && !showInlineDetail;


  return (
    <div
      className={`run-error${disconnect ? " is-disconnect" : ""}${interrupted ? " is-interrupted" : ""}${detailOpen ? " is-detail-open" : ""}`}
      role="alert"
    >
      <div className="run-error-head">
        <div className="run-error-summary">
          <CircleAlert size={14} aria-hidden />
          <div className="run-error-copy">
            <span className="run-error-text">{message}</span>
            {showInlineDetail && <span className="run-error-inline-detail">{detailText}</span>}
          </div>
        </div>
        <div className="run-error-actions">
          {showDetailToggle && (
            <ErrorDetailToggle
              detail={detailText}
              toggleOnly
              open={detailOpen}
              onOpenChange={setDetailOpen}
            />
          )}
          {onRetry && (
            <Button className="run-error-retry" variant="secondary" onClick={onRetry}>
              <RotateCcw size={12} />
              <span>{t("Retry", "重试")}</span>
            </Button>
          )}
        </div>
      </div>
      {showDetailToggle && detailOpen ? <pre className="run-error-detail-body">{detailText}</pre> : null}
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
  if (error instanceof ApiError) {
    // 优先完整 detail，避免只显示本地化摘要
    return error.detail?.trim() || error.rawMessage;
  }
  return error.stack?.trim() || error.message;
}
