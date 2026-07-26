import { RotateCcw } from "lucide-react";
import { useI18n } from "../../../features/i18n/use-i18n";

type ErrorFallbackProps = {
  error: Error;
  /** 出错区域名称，帮助用户定位是哪一块失效 */
  label?: string;
  onRetry: () => void;
};

/**
 * 渲染错误边界的默认降级内容。
 *
 * 单独抽成函数组件是为了能用 i18n——错误边界本身必须是类组件，取不到 Hook。
 *
 * @param props 异常、区域名称与重试回调
 * @returns 错误卡片
 */
export function ErrorFallback({ error, label, onRetry }: ErrorFallbackProps) {
  const { t } = useI18n();
  return (
    <div className="ui-error-boundary" role="alert">
      <strong>{label ?? t("This section failed to render", "该区域渲染失败")}</strong>
      <p>{error.message}</p>
      <button type="button" onClick={onRetry}>
        <RotateCcw size={13} />
        {t("Retry", "重试")}
      </button>
    </div>
  );
}
