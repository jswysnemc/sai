import type { HookHttpRequest, HookItem } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import { ChoicePills, FieldGrid, SettingsField, SettingsPanel, SkTextArea, SkTextInput } from "../kit";
import { KeyValueEditor } from "../key-value-editor";

type Props = { hook: HookItem; onChange: (patch: Partial<HookItem>) => void };

/**
 * 【Hooks】【动作编辑】编辑命令或全部 HTTP 请求，保留未修改请求。
 * @param props 钩子与草稿更新回调
 * @returns 动作字段
 */
export function HookActionFields({ hook, onChange }: Props) {
  const { t } = useI18n();
  if ((hook.kind ?? "command") === "command") return <SettingsPanel title={t("Shell action", "Shell 动作")}>
    <SettingsField label={t("Script", "脚本")} anchor="hooks.script" hint={t("Available variables: SAI_HOOK_EVENT, SAI_HOOK_NAME, SAI_SESSION_ID, SAI_WORKDIR, SAI_TOOL_NAME (tool events). Runs with sh -lc.", "可用变量：SAI_HOOK_EVENT、SAI_HOOK_NAME、SAI_SESSION_ID、SAI_WORKDIR、SAI_TOOL_NAME（工具事件）。通过 sh -lc 执行。")}> 
      <SkTextArea mono rows={6} value={hook.script ?? ""} onChange={(script) => onChange({ script })} />
    </SettingsField>
  </SettingsPanel>;
  const requests = hook.requests?.length ? hook.requests : [{ id: "1", url: "", method: "POST", headers: {}, body: "" }];
  return <>{requests.map((request, index) => {
    /** 更新指定请求并保留其余请求；返回无值。 */
    const patchRequest = (patch: Partial<HookHttpRequest>) => onChange({ requests: requests.map((item, i) => i === index ? { ...item, ...patch } : item) });
    return <SettingsPanel key={index} title={t(`HTTP action ${index + 1}`, `HTTP 动作 ${index + 1}`)}>
      <FieldGrid>
        <SettingsField label="URL" anchor={index === 0 ? "hooks.requests" : undefined} span="full" hint={t("When the body is empty, send the default event JSON.", "请求体为空时发送默认事件 JSON。")}><SkTextInput mono value={request.url} onChange={(url) => patchRequest({ url })} /></SettingsField>
        <SettingsField label={t("Method", "方法")} span="full"><ChoicePills value={(request.method ?? "POST").toUpperCase()} options={["GET", "POST", "PUT", "PATCH", "DELETE"].map((value) => ({ value, label: value }))} onChange={(method) => patchRequest({ method })} /></SettingsField>
        <SettingsField label={t("Body template", "请求体模板")} span="full"><SkTextArea mono rows={4} value={request.body ?? ""} onChange={(body) => patchRequest({ body })} /></SettingsField>
        <SettingsField label={t("Headers", "请求头")} span="full"><KeyValueEditor value={request.headers ?? {}} keyPlaceholder={t("Header name", "请求头名称")} valuePlaceholder={t("Header value", "请求头值")} addLabel={t("Add header", "添加请求头")} onChange={(headers) => patchRequest({ headers })} /></SettingsField>
      </FieldGrid>
    </SettingsPanel>;
  })}</>;
}
