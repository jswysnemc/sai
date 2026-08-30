import { CheckCircle2, X, XCircle } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useI18n } from "../../../features/i18n/use-i18n";
import "./notify.css";

/** 成功提示自动消失时长，失败提示保持到用户关闭 */
const SUCCESS_DISMISS_DELAY = 4000;

export type ToastNotice = {
  /** 递增序号，同一条消息重复触发时也能重新播放出现动画 */
  id: number;
  message: string;
  kind: "success" | "error";
};

/**
 * 管理单条操作提示。
 *
 * 提示挂在调用方自己的容器里，不需要全局 Provider，
 * 因此新增使用方不必改动应用根节点。
 *
 * @returns 当前提示、弹出和关闭方法
 */
export function useToast() {
  const [notice, setNotice] = useState<ToastNotice | null>(null);
  const sequence = useRef(0);

  /**
   * 弹出一条提示。
   *
   * @param message 提示文案
   * @param kind 提示类型，默认成功
   * @returns 无返回值
   */
  const showToast = useCallback((message: string, kind: ToastNotice["kind"] = "success") => {
    sequence.current += 1;
    setNotice({ id: sequence.current, message, kind });
  }, []);

  const dismissToast = useCallback(() => setNotice(null), []);

  return { notice, showToast, dismissToast };
}

type ToastProps = {
  notice: ToastNotice | null;
  onDismiss: () => void;
};

/**
 * 浮出一条操作结果提示。
 *
 * 成功提示定时自动消失，失败提示保留到用户手动关闭。
 *
 * @param props 提示数据与关闭回调
 * @returns 操作结果提示条；无提示时不渲染
 */
export function Toast({ notice, onDismiss }: ToastProps) {
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
      className={`ui-toast ${notice.kind}`}
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
