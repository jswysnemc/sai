import { CheckCircle2, X, XCircle } from "lucide-react";
import { useEffect, useRef } from "react";
import { useI18n } from "../../i18n/use-i18n";
import type { OperationNotice } from "./operation-notice";
import "./git-operation-toast.css";

/** 成功提示自动消失时长，失败提示保持到用户关闭 */
const SUCCESS_DISMISS_DELAY = 4000;

type GitOperationToastProps = {
  notice: OperationNotice | null;
  onDismiss: () => void;
};

/**
 * 浮出一条 Git 操作结果提示。
 *
 * 成功提示定时自动消失，失败提示保留到用户手动关闭，
 * 完整输出仍由下方输出面板承载，此处只做短反馈。
 *
 * @param props 提示数据与关闭回调
 * @returns 操作结果提示条；无提示时不渲染
 */
export function GitOperationToast({ notice, onDismiss }: GitOperationToastProps) {
  const { t } = useI18n();
  const onDismissRef = useRef(onDismiss);
  onDismissRef.current = onDismiss;

  useEffect(() => {
    // 仅成功提示定时消失，失败信息需要留给用户阅读
    if (!notice || notice.kind !== "success") return;
    const timer = window.setTimeout(() => onDismissRef.current(), SUCCESS_DISMISS_DELAY);
    return () => window.clearTimeout(timer);
  }, [notice]);

  if (!notice) return null;

  return (
    <div
      key={notice.id}
      className={`git-operation-toast ${notice.kind}`}
      role={notice.kind === "error" ? "alert" : "status"}
      aria-live={notice.kind === "error" ? "assertive" : "polite"}
    >
      {notice.kind === "success" ? <CheckCircle2 size={14} /> : <XCircle size={14} />}
      <p>{notice.message}</p>
      <button type="button" onClick={onDismiss} aria-label={t("Dismiss notification", "关闭提示")}>
        <X size={13} />
      </button>
    </div>
  );
}
