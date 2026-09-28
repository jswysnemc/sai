import { useState } from "react";
import type { ModelMetadata } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { ChoicePills, FieldGrid, SettingsField, SkNumberInput, SkSelect } from "../kit";

const MODEL_TAGS = ["tool", "thinking", "vision", "web_search", "fast", "low_cost"];
const THINKING_LEVELS = ["none", "low", "medium", "high", "xhigh", "max"];

/**
 * 【供应商设置】【模型能力】编辑上下文、输出上限、工具策略及能力标签。
 * @param props 当前模型元数据与局部更新回调
 * @returns 模型能力字段栅格
 */
export function ModelCapabilityFields({ metadata, onChange }: { metadata: ModelMetadata; onChange: (patch: Partial<ModelMetadata>) => void }) {
  const { t } = useI18n();
  const [unit, setUnit] = useState<"none" | "k" | "m">("none");
  const divisor = unit === "k" ? 1000 : unit === "m" ? 1_000_000 : 1;

  /**
   * 切换有序集合中的一个条目，保留其他能力选择。
   * @param values 当前已选集合
   * @param value 切换的条目
   * @returns 更新后的集合
   */
  const toggle = (values: string[], value: string) => values.includes(value) ? values.filter((item) => item !== value) : [...values, value];

  return <FieldGrid>
    <SettingsField label={t("Context tokens", "上下文 token 数")} anchor="providers.models.context_chars" configKey="model_metadata.context_chars" hint={t("Choose tokens, thousands or millions.", "支持原始数量、千或百万为单位。")}>
      <div className="grid grid-cols-[minmax(0,10rem)_5rem] gap-2">
        <SkNumberInput value={metadata.context_chars === undefined ? null : metadata.context_chars / divisor} min={0} allowEmpty onChange={(value) => onChange({ context_chars: value === null ? undefined : Math.round(value * divisor) })} />
        <SkSelect value={unit} options={[{ value: "none", label: t("Tokens", "无") }, { value: "k", label: "k" }, { value: "m", label: "m" }]} onChange={setUnit} ariaLabel={t("Context unit", "上下文单位")} />
      </div>
    </SettingsField>
    <SettingsField label={t("Maximum output tokens", "最大输出 token 数")} anchor="providers.models.max_output_tokens" configKey="model_metadata.max_output_tokens" size="sm" hint={t("Applied to Chat, Responses and Anthropic requests.", "应用于 Chat、Responses 和 Anthropic 请求。")}>
      <SkNumberInput value={metadata.max_output_tokens} min={1} integer allowEmpty onChange={(value) => onChange({ max_output_tokens: value ?? undefined })} />
    </SettingsField>
    <SettingsField label={t("Tool calls", "工具调用")} anchor="providers.models.tools_enabled" configKey="model_metadata.tools_enabled">
      <ChoicePills value={metadata.tools_enabled === false ? "disabled" : "enabled"} options={[{ value: "enabled", label: t("Allowed", "允许") }, { value: "disabled", label: t("Disabled", "禁用") }]} onChange={(value) => onChange({ tools_enabled: value === "enabled" ? undefined : false })} />
    </SettingsField>
    <SettingsField label={t("Web search tool", "网页搜索工具")} anchor="providers.models.web_search_tool_mode" configKey="model_metadata.web_search_tool_mode">
      <SkSelect value={metadata.web_search_tool_mode ?? "enabled"} options={[{ value: "enabled", label: t("Enabled", "启用") }, { value: "hide_builtin", label: t("Hide local tool", "隐藏本地同名工具") }, { value: "rename_local", label: t("Rename local tool", "更名本地工具") }]} onChange={(value) => onChange({ web_search_tool_mode: value === "enabled" ? undefined : value })} />
    </SettingsField>
    <SettingsField label={t("Supported reasoning levels", "支持的推理强度")} anchor="providers.models.thinking_levels" configKey="model_metadata.thinking_levels" span="full" hint={t("No selection allows every level. Auto is always available.", "不选择时允许全部等级，自动模式始终可用。")}>
      <div className="flex flex-wrap gap-1">
        {THINKING_LEVELS.map((level) => <Button variant="ghost" size="small" key={level} aria-pressed={metadata.thinking_levels?.includes(level) ?? false} className={metadata.thinking_levels?.includes(level) ? "active" : undefined} onClick={() => { const next = toggle(metadata.thinking_levels ?? [], level); onChange({ thinking_levels: THINKING_LEVELS.filter((item) => next.includes(item)) }); }}>{level}</Button>)}
        {!!metadata.thinking_levels?.length && <Button size="small" onClick={() => onChange({ thinking_levels: undefined })}>{t("Clear", "清空")}</Button>}
      </div>
    </SettingsField>
    <SettingsField label={t("Model tags", "模型标签")} anchor="providers.models.tags" configKey="model_metadata.tags" span="full">
      <div className="flex flex-wrap gap-1">
        {MODEL_TAGS.map((tag) => <Button variant="ghost" size="small" key={tag} aria-pressed={metadata.tags?.includes(tag) ?? false} className={metadata.tags?.includes(tag) ? "active" : undefined} onClick={() => onChange({ tags: toggle(metadata.tags ?? [], tag) })}>{tag}</Button>)}
      </div>
    </SettingsField>
  </FieldGrid>;
}
