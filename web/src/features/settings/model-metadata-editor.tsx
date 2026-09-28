import { Plus, Trash2 } from "../../shared/ui/icons";
import { useEffect, useState } from "react";
import type { ModelMetadata, ProviderConfig } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { ModelIcon } from "../../shared/ui/model-icon";
import { DetailHeader, EmptyGuide, MasterDetail, ObjectList, SettingsPanel, SkTextInput } from "./kit";
import { ModelCapabilityFields } from "./model/model-capability-fields";
import { ModelMetadataPreview } from "./model/model-metadata-preview";
import { ProviderConnectionTest } from "./model/provider-connection-test";
import { useI18n } from "../i18n/use-i18n";

type ModelMetadataEditorProps = {
  provider: ProviderConfig;
  onChange: (patch: Partial<ProviderConfig>) => void;
};

/**
 * 渲染模型列表、默认模型和单模型能力元数据。
 * 限制与能力字段放在同一表单，不使用分段切换。
 *
 * @param props 供应商配置和更新回调
 * @returns 模型目录编辑器
 */
export function ModelMetadataEditor({ provider, onChange }: ModelMetadataEditorProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const models = provider.models ?? [];
  const [selected, setSelected] = useState(provider.default_model || models[0] || "");
  const [draft, setDraft] = useState("");

  useEffect(() => {
    if (!models.includes(selected)) setSelected(provider.default_model || models[0] || "");
  }, [models.join("\u0000"), provider.default_model, selected]);

  const metadata = provider.model_metadata?.[selected] ?? {};
  /**
   * 新增模型标识并选中。
   */
  const addModel = () => {
    const model = draft.trim();
    if (!model || models.includes(model)) return;
    onChange({ models: [...models, model], default_model: provider.default_model || model });
    setSelected(model);
    setDraft("");
  };

  /**
   * 删除指定模型及其元数据。
   *
   * 删除同时会清掉该模型的能力配置，且无法撤销，因此先确认一次。
   *
   * @param model 模型标识
   * @returns 确认流程完成后返回
   */
  const removeModel = async (model: string) => {
    const confirmed = await confirm({
      title: t("Delete model", "删除模型"),
      description: t(
        `Delete “${model}” and its capability settings from this provider.`,
        `将从该供应商删除“${model}”及其能力配置。`
      ),
      confirmLabel: t("Delete model", "删除模型"),
      danger: true
    });
    if (!confirmed) return;
    const nextModels = models.filter((item) => item !== model);
    const nextMetadata = { ...(provider.model_metadata ?? {}) };
    delete nextMetadata[model];
    onChange({
      models: nextModels,
      default_model: provider.default_model === model ? nextModels[0] ?? "" : provider.default_model,
      model_metadata: nextMetadata
    });
  };

  /**
   * 更新当前选中模型的元数据字段。
   *
   * @param patch 元数据局部更新
   */
  const updateMetadata = (patch: Partial<ModelMetadata>) => {
    if (!selected) return;
    onChange({
      model_metadata: {
        ...(provider.model_metadata ?? {}),
        [selected]: { ...metadata, ...patch }
      }
    });
  };

  return (
    <SettingsPanel title={t("Model catalog", "模型目录")} description={t(`${models.length} configured models`, `已配置 ${models.length} 个模型`)} actions={
      <div className="flex min-w-0 items-center gap-2">
        <SkTextInput value={draft} onChange={setDraft} placeholder={t("Add model ID", "新增模型 ID")} aria-label={t("Add model ID", "新增模型 ID")} onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); addModel(); } }} />
        <Button size="icon" onClick={addModel} aria-label={t("Add model", "新增模型")} disabled={!draft.trim() || models.includes(draft.trim())}><Plus size={14} /></Button>
      </div>
    }>
      {!models.length ? <EmptyGuide title={t("No models yet", "尚未配置模型")} description={t("Enter a model ID above or import the remote catalog.", "在上方填写模型标识，或导入远端模型目录。")} /> : (
        <MasterDetail list={<ObjectList title={t("Models", "模型")} items={models.map((model) => ({ id: model, name: model, marked: model === provider.default_model, icon: <ModelIcon model={model} size={14} /> }))} selectedId={selected} onSelect={setSelected} searchPlaceholder={t("Filter models", "筛选模型")} />}>
          <DetailHeader title={selected} subtitle={t("Model capabilities and context", "单模型能力与上下文")} actions={<Button size="small" disabled={provider.default_model === selected} onClick={() => onChange({ default_model: selected })}>{provider.default_model === selected ? t("Default model", "默认模型") : t("Set as default", "设为默认")}</Button>} menuItems={[{ id: "delete", label: t(`Delete model ${selected}`, `删除模型 ${selected}`), icon: <Trash2 size={14} />, danger: true, onSelect: () => void removeModel(selected) }]} />
          <ModelCapabilityFields key={selected} metadata={metadata} onChange={updateMetadata} />
          <SettingsPanel title={t("Test selected model", "测试当前模型")} description={t("Uses this model and the selected provider key; the default model stays unchanged.", "使用当前模型与供应商所选密钥，不修改默认模型。")}>
            <ProviderConnectionTest provider={provider} model={selected} selectedKeyId={provider.api_key_selected} />
          </SettingsPanel>
          <ModelMetadataPreview model={selected} metadata={metadata} />
        </MasterDetail>
      )}
    </SettingsPanel>
  );
}
