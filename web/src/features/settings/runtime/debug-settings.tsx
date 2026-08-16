import type { AppConfig } from "../../../api/contracts";
import { SettingsGroup } from "../editor-layout";
import { useI18n } from "../../i18n/use-i18n";

type DebugSettingsProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/** 配置 Web 可控的 API 请求与响应调试记录。 */
export function DebugSettings({ config, onConfigChange }: DebugSettingsProps) {
  const { t } = useI18n();
  const debug = config.debug ?? { enabled: false, retain_logs: true };
  const patch = (next: Partial<typeof debug>) => onConfigChange({
    ...config,
    debug: { ...debug, ...next }
  });

  return (
    <SettingsGroup
      title={t("API debugging", "API 调试")}
      description={t(
        "Keep the exact request body, response stream, response headers, and detailed provider metadata for troubleshooting. These records can contain sensitive context.",
        "保留完整请求体、响应流、响应头和供应商元数据，便于排查问题。记录可能包含敏感上下文。"
      )}
    >
      <div className="settings-form-grid">
        <label className="settings-toggle-field">
          <span>
            <strong>{t("Enable API debug", "开启 API 调试")}</strong>
            <small>{t("Record real provider requests and responses for each session.", "为每个会话记录真实供应商请求与响应。")}</small>
          </span>
          <input type="checkbox" checked={debug.enabled} onChange={(event) => patch({ enabled: event.target.checked })} />
        </label>
        <label className="settings-toggle-field">
          <span>
            <strong>{t("Retain complete debug logs", "保留完整调试日志")}</strong>
            <small>{t("Keep request bodies and raw SSE responses instead of only summaries.", "保留请求体和原始 SSE 响应，不只保留摘要。")}</small>
          </span>
          <input type="checkbox" checked={debug.retain_logs !== false} onChange={(event) => patch({ retain_logs: event.target.checked })} />
        </label>
      </div>
    </SettingsGroup>
  );
}
