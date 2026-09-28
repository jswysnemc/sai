import type { AppConfig } from "../../../api/contracts";
import { DiamondCheck, ShieldCheck } from "../../../shared/ui/icons";
import type { SelectOption } from "../../../shared/ui/select/select";
import { useI18n } from "../../i18n/use-i18n";
import { FieldGrid, SettingsField, SettingsPanel, SkSelect, SwitchField } from "../kit";
import { JevConnectionStatus } from "./jev-connection-status";
import { JevNumberField } from "./jev-number-field";
import { jevEndpoints, patchJevAudit, patchJevRouting, readJevConfig, selectJevEndpoint } from "./jev-config";

type JevFeatureSettingsProps = {
  config: AppConfig;
  dirty: boolean;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 【Jev设置】【功能页】选择接入并开关工具与 Skills 暴露决策、权限自动审核。
 * @param props 应用配置、草稿修改状态与更新回调
 * @returns 接入、暴露决策与权限审核三个分组
 */
export function JevFeatureSettings({ config, dirty, onConfigChange }: JevFeatureSettingsProps) {
  const { t } = useI18n();
  const jev = readJevConfig(config);
  const endpoints = jevEndpoints(config);
  const pluginAudit = (config.permission?.auto_audit_plugin_id ?? "").trim();
  const endpointOptions: SelectOption<string>[] = [
    {
      value: "",
      label: t("Automatic", "自动"),
      description: t("First Jev connection; without one, the official TypeSafe endpoint and TYPESAFE_API_KEY", "第一条 Jev 接入；没有接入时使用 TypeSafe 官方地址与 TYPESAFE_API_KEY")
    },
    ...endpoints.map((item) => ({ value: item.id, label: item.name, description: item.endpoint }))
  ];

  return (
    <div className="grid gap-4">
      <SettingsPanel title={t("Connection", "接入")} description={t("Both features share one connection; edit its details below.", "两项功能共用一个接入，地址与密钥在下方维护。")}>
        <FieldGrid>
          <SettingsField label={t("Jev connection", "Jev 接入")} anchor="jev.endpoint_id" configKey="jev.endpoint_id"><SkSelect value={jev.endpoint_id} options={endpointOptions} onChange={(value) => onConfigChange(selectJevEndpoint(config, value))} /></SettingsField>
        </FieldGrid>
        <JevConnectionStatus dirty={dirty} />
      </SettingsPanel>

      <div className="jev-feature-grid grid grid-cols-1 items-start gap-4 xl:grid-cols-2">
      <SettingsPanel
        title={t("Tool & skill routing", "工具与 Skills 暴露决策")}
        icon={<DiamondCheck size={14} />}
        description={t("Before each request Jev picks which tools and skills to expose. The model can ask for more with request_capability. Basic tools stay exposed.", "每次请求前由 Jev 决定暴露哪些工具与 Skills，模型可通过 request_capability 追加申请；基础工具始终暴露。")}
      >
        <SwitchField label={t("Enable routing", "启用暴露决策")} hint={t("Applies to new turns in Web and TUI sessions. DeepSeek anchored mode takes precedence when both are on.", "对 Web 与 TUI 会话的新一轮生效；与 DeepSeek 锚定模式同时开启时以锚定模式为准。")} anchor="jev.routing.enabled" configKey="jev.routing.enabled" checked={jev.routing.enabled} onChange={(enabled) => onConfigChange(patchJevRouting(config, { enabled }))} />
        <FieldGrid>
            <JevNumberField label={t("Minimum probability", "最低概率")} hint={t("Candidates below this Noul probability are not exposed.", "Noul 概率低于该值的候选不暴露。")} value={jev.routing.threshold} anchor="jev.routing.threshold" disabled={!jev.routing.enabled} min={0} max={1} step={0.05} onChange={(threshold) => onConfigChange(patchJevRouting(config, { threshold }))} />
            <JevNumberField label={t("Max tools per decision", "单次最多工具数")} value={jev.routing.max_tools} anchor="jev.routing.max_tools" disabled={!jev.routing.enabled} min={0} max={50} integer onChange={(max_tools) => onConfigChange(patchJevRouting(config, { max_tools }))} />
            <JevNumberField label={t("Max skills per decision", "单次最多 Skills 数")} value={jev.routing.max_skills} anchor="jev.routing.max_skills" disabled={!jev.routing.enabled} min={0} max={50} integer onChange={(max_skills) => onConfigChange(patchJevRouting(config, { max_skills }))} />
            <JevNumberField label={t("Timeout (seconds)", "超时（秒）")} value={jev.routing.timeout_seconds} anchor="jev.routing.timeout_seconds" disabled={!jev.routing.enabled} min={1} max={60} integer onChange={(timeout_seconds) => onConfigChange(patchJevRouting(config, { timeout_seconds }))} />
            <JevNumberField label={t("Context characters", "判断所用对话字符数")} hint={t("Recent conversation sent as background for each decision.", "每次判断附带的近期对话长度。")} value={jev.routing.context_chars} anchor="jev.routing.context_chars" disabled={!jev.routing.enabled} min={200} max={20000} integer onChange={(context_chars) => onConfigChange(patchJevRouting(config, { context_chars }))} />
        </FieldGrid>
      </SettingsPanel>

      <SettingsPanel
        title={t("Permission audit", "权限自动审核")}
        icon={<ShieldCheck size={14} />}
        description={t("In auto-audit permission mode Jev reviews each pending operation with a Choice question. Uncertain answers and failures go to human review.", "自动审核权限模式下由 Jev 以 Choice 问题审核每个待批操作；判断不确定或调用失败时交还人工。")}
      >
        <SwitchField
          label={t("Enable Jev audit", "启用 Jev 审核")}
          hint={pluginAudit && !jev.audit.enabled
            ? t(`Auto-audit currently uses the plugin ${pluginAudit}. Enabling this replaces it.`, `自动审核当前使用插件 ${pluginAudit}，开启后改用内置审核。`)
            : t("Takes precedence over audit plugins and the chat-model audit. Switch the session to auto-audit mode to use it.", "优先于审核插件与聊天模型审核；会话需切换到自动审核模式。")}
          anchor="jev.audit.enabled" configKey="jev.audit.enabled" checked={jev.audit.enabled}
          onChange={(enabled) => onConfigChange(patchJevAudit(config, { enabled }))}
        />
        <FieldGrid>
            <JevNumberField label={t("Minimum choice probability", "最低选项概率")} value={jev.audit.minimum_probability} anchor="jev.audit.minimum_probability" disabled={!jev.audit.enabled} min={0.5} max={1} step={0.01} onChange={(minimum_probability) => onConfigChange(patchJevAudit(config, { minimum_probability }))} />
            <JevNumberField label={t("Minimum confidence", "最低置信度")} value={jev.audit.minimum_confidence} anchor="jev.audit.minimum_confidence" disabled={!jev.audit.enabled} min={0.5} max={1} step={0.01} onChange={(minimum_confidence) => onConfigChange(patchJevAudit(config, { minimum_confidence }))} />
            <JevNumberField label={t("Timeout (seconds)", "超时（秒）")} value={jev.audit.timeout_seconds} anchor="jev.audit.timeout_seconds" disabled={!jev.audit.enabled} min={1} max={30} integer onChange={(timeout_seconds) => onConfigChange(patchJevAudit(config, { timeout_seconds }))} />
        </FieldGrid>
      </SettingsPanel>
      </div>
    </div>
  );
}
