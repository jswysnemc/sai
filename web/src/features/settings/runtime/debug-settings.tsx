import { useI18n } from "../../i18n/use-i18n";
import { FieldGrid, SettingsPanel, SwitchField } from "../kit";
import type { RuntimeSettingsProps } from "./runtime-settings-types";

/**
 * 【Web 设置】【请求调试】配置请求与响应记录及日志保留策略。
 * @param props 应用配置与更新回调
 * @returns 调试设置面板
 */
export function DebugSettings({ config, onConfigChange }: RuntimeSettingsProps) {
  const { t } = useI18n();
  const debug = config.debug ?? { enabled: false, retain_logs: true };

  /**
   * 合并调试开关，保留其他调试字段。
   * @param patch 修改的调试字段
   * @returns 无返回值
   */
  const update = (patch: Partial<typeof debug>) => onConfigChange({ ...config, debug: { ...debug, ...patch } });

  return (
    <SettingsPanel title={t("API debugging", "API 调试")} description={t("Record requests, responses, headers, and provider metadata. Logs may include sensitive context.", "记录请求、响应、响应头与供应商元数据。日志可能包含敏感上下文。")}>
      <FieldGrid>
        <SwitchField label={t("Enable API debug", "开启 API 调试")} hint={t("Record provider requests and responses for each session.", "为每个会话记录供应商请求与响应。")} checked={debug.enabled} anchor="runtime.debug.enabled" configKey="debug.enabled" onChange={(value) => update({ enabled: value })} />
        <SwitchField label={t("Retain complete debug logs", "保留完整调试日志")} hint={t("Keep request bodies and raw SSE responses as well as summaries.", "除摘要外，还保留请求体与原始 SSE 响应。")} checked={debug.retain_logs !== false} anchor="runtime.debug.retain_logs" configKey="debug.retain_logs" onChange={(value) => update({ retain_logs: value })} />
      </FieldGrid>
    </SettingsPanel>
  );
}
