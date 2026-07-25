import { RefreshCw } from "lucide-react";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import "./settings-error-recovery.css";

type SettingsErrorRecoveryProps = {
  /** 错误信息 */
  message: string;
  /** 重试回调 */
  onRetry: () => void;
};

/**
 * 设置页加载失败时的错误提示与重试按钮。
 *
 * @param props 错误信息与重试回调
 * @returns 错误恢复面板
 */
export function SettingsErrorRecovery({ message, onRetry }: SettingsErrorRecoveryProps) {
  const { t } = useI18n();
  return (
    <div className="settings-error-recovery" role="alert">
      <p className="settings-error-recovery-message">{message}</p>
      <Button variant="secondary" onClick={onRetry}>
        <RefreshCw size={14} />
        {t("Retry", "重试")}
      </Button>
    </div>
  );
}
