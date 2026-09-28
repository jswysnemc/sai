import type { RetryConfig } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import { ChoicePills, FieldGrid, SettingsField, SkNumberInput } from "../kit";
import type { RuntimeSettingsProps } from "./runtime-settings-types";

/**
 * 【Web 设置】【失败重试】配置输出开始前瞬时传输故障的重试次数与间隔。
 * @param props 应用配置与更新回调
 * @returns 重试字段栅格
 */
export function RetrySettings({ config, onConfigChange }: RuntimeSettingsProps) {
  const { t } = useI18n();
  const retry = config.retry ?? {};

  /**
   * 合并重试配置补丁。
   * @param patch 修改的重试字段
   * @returns 无返回值
   */
  const update = (patch: Partial<RetryConfig>) => onConfigChange({ ...config, retry: { ...retry, ...patch } });

  return (
    <FieldGrid columns={3}>
      <SettingsField label={t("Max attempts", "最大尝试次数")} hint={t("Total requests, including the first attempt.", "包含首次请求的总尝试次数。")} anchor="runtime.retry.max_attempts" configKey="retry.max_attempts" size="xs">
        <SkNumberInput value={retry.max_attempts ?? 3} min={1} max={10} integer onChange={(value) => update({ max_attempts: value ?? 3 })} />
      </SettingsField>
      <SettingsField label={t("Initial delay", "首次重试间隔")} hint={t("Wait before the first retry.", "第一次重试前的等待时间。")} anchor="runtime.retry.initial_delay_ms" configKey="retry.initial_delay_ms" size="sm">
        <SkNumberInput value={retry.initial_delay_ms ?? 200} min={0} max={60_000} integer step={100} unit="ms" onChange={(value) => update({ initial_delay_ms: value ?? 200 })} />
      </SettingsField>
      <SettingsField label={t("Delay schedule", "间隔方式")} hint={t("Exponential doubles the delay; fixed keeps it constant.", "指数退避每次翻倍，固定间隔保持不变。")} anchor="runtime.retry.backoff" configKey="retry.backoff">
        <ChoicePills value={retry.backoff === "fixed" ? "fixed" : "exponential"} options={[{ value: "exponential", label: t("Exponential", "指数退避") }, { value: "fixed", label: t("Fixed", "固定间隔") }]} onChange={(value) => update({ backoff: value })} />
      </SettingsField>
    </FieldGrid>
  );
}
