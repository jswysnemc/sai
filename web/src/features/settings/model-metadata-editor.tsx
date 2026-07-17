import { Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import type { ModelMetadata, ProviderConfig } from "../../api/contracts";
import { Select } from "../../shared/ui/select/select";
import { useI18n } from "../i18n/use-i18n";

const MODEL_TAGS = ["tool", "thinking", "vision", "web_search", "fast", "low_cost"];

type ModelMetadataEditorProps = {
  provider: ProviderConfig;
  onChange: (patch: Partial<ProviderConfig>) => void;
};

/**
 * 渲染模型列表、默认模型和单模型能力元数据。
 *
 * @param props 供应商配置和更新回调
 * @returns 模型目录编辑器
 */
export function ModelMetadataEditor({ provider, onChange }: ModelMetadataEditorProps) {
  const { t } = useI18n();
  const models = provider.models ?? [];
  const [selected, setSelected] = useState(provider.default_model || models[0] || "");
  const [draft, setDraft] = useState("");

  useEffect(() => {
    if (!models.includes(selected)) setSelected(provider.default_model || models[0] || "");
  }, [models.join("\u0000"), provider.default_model, selected]);

  const metadata = provider.model_metadata?.[selected] ?? {};
  const [contextUnit, setContextUnit] = useState<"none" | "k" | "m">("none");
  const contextDivisor = contextUnit === "k" ? 1_000 : contextUnit === "m" ? 1_000_000 : 1;
  const contextValue = metadata.context_chars ? metadata.context_chars / contextDivisor : "";
  const toolOptions = [
    { value: "enabled", label: t("Allowed", "允许") },
    { value: "disabled", label: t("Disabled", "禁用") }
  ];
  const contextUnitOptions = [
    { value: "none", label: t("None", "无") },
    { value: "k", label: "k" },
    { value: "m", label: "m" }
  ];
  const webSearchToolOptions = [
    { value: "hide_builtin", label: t("Hide local tool with the same name", "隐藏本地同名工具") },
    { value: "rename_local", label: t("Rename local tool", "更名本地工具") }
  ];

  /** 新增模型标识并选中。 */
  const addModel = () => {
    const model = draft.trim();
    if (!model || models.includes(model)) return;
    onChange({ models: [...models, model], default_model: provider.default_model || model });
    setSelected(model);
    setDraft("");
  };

  /** 删除当前模型及其元数据。 */
  const removeModel = (model: string) => {
    const nextModels = models.filter((item) => item !== model);
    const nextMetadata = { ...(provider.model_metadata ?? {}) };
    delete nextMetadata[model];
    onChange({
      models: nextModels,
      default_model: provider.default_model === model ? nextModels[0] ?? "" : provider.default_model,
      model_metadata: nextMetadata
    });
  };

  /** 更新当前模型元数据。 */
  const updateMetadata = (patch: Partial<ModelMetadata>) => {
    if (!selected) return;
    onChange({
      model_metadata: {
        ...(provider.model_metadata ?? {}),
        [selected]: { ...metadata, ...patch }
      }
    });
  };

  /** 切换当前模型标签。 */
  const toggleTag = (tag: string) => {
    const tags = metadata.tags ?? [];
    updateMetadata({ tags: tags.includes(tag) ? tags.filter((item) => item !== tag) : [...tags, tag] });
  };

  return (
    <section className="model-catalog">
      <header><div><span>{t("Model catalog", "模型目录")}</span><small>{t(`${models.length} models`, `${models.length} 个模型`)}</small></div><div className="model-add"><input value={draft} onChange={(event) => setDraft(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); addModel(); } }} placeholder={t("Add model ID", "新增模型 ID")} /><button type="button" onClick={addModel} aria-label={t("Add model", "新增模型")}><Plus size={14} /></button></div></header>
      <div className="model-catalog-body">
        <div className="model-chip-list">
          {models.map((model) => (
            <div className={model === selected ? "model-chip active" : "model-chip"} key={model}>
              <button type="button" onClick={() => setSelected(model)}>{model}</button>
              <button type="button" onClick={() => removeModel(model)} aria-label={t(`Delete model ${model}`, `删除模型 ${model}`)}><Trash2 size={12} /></button>
            </div>
          ))}
        </div>
        {selected && (
          <div className="model-metadata-form">
            <div className="model-metadata-head"><div><strong>{selected}</strong><small>{t("Model capabilities and context", "单模型能力与上下文")}</small></div><button type="button" className={provider.default_model === selected ? "settings-secondary active" : "settings-secondary"} onClick={() => onChange({ default_model: selected })}>{provider.default_model === selected ? t("Default model", "默认模型") : t("Set as default", "设为默认")}</button></div>
            <div className="settings-form-grid">
              <div className="settings-field"><span>{t("Context tokens", "上下文 token 数")}</span><div className="model-context-input"><input type="number" min="0" step="any" value={contextValue} onChange={(event) => updateMetadata({ context_chars: event.target.value ? Math.round(Number(event.target.value) * contextDivisor) : undefined })} placeholder={t("For example, 128", "例如 128")} /><Select value={contextUnit} options={contextUnitOptions} onChange={(value) => setContextUnit(value as "none" | "k" | "m")} ariaLabel={t("Context unit", "上下文单位")} /></div><small>{t("Supports no unit, k, or m", "支持无单位、k、m")}</small></div>
              <div className="settings-field"><span>{t("Tool calls", "工具调用")}</span><Select value={metadata.tools_enabled === false ? "disabled" : "enabled"} options={toolOptions} onChange={(value) => updateMetadata({ tools_enabled: value === "enabled" ? undefined : false })} ariaLabel={t("Model tool calls", "模型工具调用")} /><small>{t("Override the provider default capability", "覆盖供应商默认能力")}</small></div>
            </div>
            <div className="model-tag-field"><span>{t("Model tags", "模型标签")}</span><div>{MODEL_TAGS.map((tag) => <button type="button" className={(metadata.tags ?? []).includes(tag) ? "active" : ""} key={tag} onClick={() => toggleTag(tag)}>{tag}</button>)}</div></div>
            {(metadata.tags ?? []).includes("web_search") && <div className="settings-field"><span>{t("Web search tool conflict", "网页搜索工具冲突")}</span><Select value={metadata.web_search_tool_mode ?? "hide_builtin"} options={webSearchToolOptions} onChange={(value) => updateMetadata({ web_search_tool_mode: value as ModelMetadata["web_search_tool_mode"] })} ariaLabel={t("Web search tool conflict policy", "网页搜索工具冲突策略")} /><small>{t("Hide the local tool with the same name, or rename it before sending", "隐藏同名本地工具，或将本地工具更名后发送")}</small></div>}
          </div>
        )}
      </div>
    </section>
  );
}
