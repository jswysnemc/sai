import type { AppConfig, RetryConfig } from "../../../api/contracts";
import { Select, type SelectOption } from "../../../shared/ui/select/select";
import { useI18n } from "../../i18n/use-i18n";

type RetrySettingsProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

const DEFAULT_MAX_ATTEMPTS = 3;
const DEFAULT_INITIAL_DELAY_MS = 200;

/**
 * 模型请求瞬时失败的自动重试策略。
 *
 * 只对输出开始前的瞬时传输故障生效；鉴权、余额、上下文超限等业务错误
 * 与已开始输出的请求不会重试。这条边界写在描述里，避免用户误以为
 * 重试能救回所有失败。
 *
 * @param props 应用配置与更新回调
 * @returns 重试设置字段组
 */
export function RetrySettings({ config, onConfigChange }: RetrySettingsProps) {
  const { t } = useI18n();
  const retry: RetryConfig = config.retry ?? {};

  /**
   * 合并补丁并回写重试配置。
   *
   * @param patch 待合并的重试字段
   * @returns 无返回值
   */
  const updateRetry = (patch: Partial<RetryConfig>) => {
    onConfigChange({
      ...config,
      retry: { ...retry, ...patch }
    });
  };

  const backoff = retry.backoff === "fixed" ? "fixed" : "exponential";
  const backoffOptions: SelectOption<"exponential" | "fixed">[] = [
    {
      value: "exponential",
      label: t("Exponential backoff", "指数退避"),
      description: t("Doubles the delay after each failed attempt", "每次重试后间隔翻倍")
    },
    {
      value: "fixed",
      label: t("Fixed interval", "固定间隔"),
      description: t("Waits the same delay before every attempt", "每次重试等待相同时间")
    }
  ];

  return (
    <div className="settings-form-grid">
      <label className="settings-field">
        <span>{t("Max attempts", "最大尝试次数")}</span>
        <input
          type="number"
          min={1}
          max={10}
          step={1}
          value={retry.max_attempts ?? DEFAULT_MAX_ATTEMPTS}
          onChange={(event) => updateRetry({
            max_attempts: Math.min(10, Math.max(1, Math.round(Number(event.target.value) || 1)))
          })}
        />
        <small>{t("Total requests per turn including the first one. Only transient transport failures are retried.", "每轮请求的总尝试次数（含首次）。仅瞬时传输故障会重试。")}</small>
      </label>
      <label className="settings-field">
        <span>{t("Initial delay", "首次重试间隔")}</span>
        <input
          type="number"
          min={0}
          max={60_000}
          step={100}
          value={retry.initial_delay_ms ?? DEFAULT_INITIAL_DELAY_MS}
          onChange={(event) => updateRetry({
            initial_delay_ms: Math.min(60_000, Math.max(0, Math.round(Number(event.target.value) || 0)))
          })}
        />
        <small>{t("Milliseconds to wait before the first retry.", "第一次重试前等待的毫秒数。")}</small>
      </label>
      <label className="settings-field">
        <span>{t("Delay schedule", "间隔方式")}</span>
        <Select
          value={backoff}
          options={backoffOptions}
          onChange={(value) => updateRetry({ backoff: value })}
          ariaLabel={t("Delay schedule", "间隔方式")}
        />
        <small>{t("Exponential doubles the delay each retry; fixed keeps it constant.", "指数退避每次翻倍；固定间隔保持不变。")}</small>
      </label>
    </div>
  );
}
