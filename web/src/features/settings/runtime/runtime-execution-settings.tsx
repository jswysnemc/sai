import { useI18n } from "../../i18n/use-i18n";
import { SettingsPanel } from "../kit";
import { AgentEngineSettings } from "./agent-engine-settings";
import { RetrySettings } from "./retry-settings";
import { SessionTitleSettings } from "./session-title-settings";
import type { RuntimeSettingsProps } from "./runtime-settings-types";

/**
 * 【Web 设置】【执行与会话】组合内核、新会话默认值、标题与重试策略。
 * @param props 应用配置与更新回调
 * @returns 执行与会话设置子页
 */
export function RuntimeExecutionSettings(props: RuntimeSettingsProps) {
  const { t } = useI18n();
  return (
    <>
      <SettingsPanel title={t("Engine and new sessions", "内核与新会话")} description={t("Choose how new conversations run.", "配置新会话使用的内核与默认参数。")}>
        <AgentEngineSettings {...props} />
      </SettingsPanel>
      <SessionTitleSettings {...props} />
      <SettingsPanel title={t("Failure retry", "失败重试")} description={t("Retry transient transport failures before output starts; business errors and active streams are not retried.", "仅在输出开始前重试瞬时传输故障；业务错误与已开始输出的请求不会重试。")}>
        <RetrySettings {...props} />
      </SettingsPanel>
    </>
  );
}
