import { Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../../api/client";
import { toDisplayError } from "../../../api/api-error";
import type { AppConfig, ModelEndpointConfig, ModelEndpointKind } from "../../../api/contracts/config";
import { Button } from "../../../shared/ui/button/button";
import { PasswordField } from "../../../shared/ui/password-field";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { useI18n } from "../../i18n/use-i18n";
import { EditorHeader, SettingsGroup } from "../editor-layout";
import { ObjectListPanel } from "../object-list-panel";
import { TextFieldRow } from "../controls/field-row";
import { newModelEndpoint, updateModelEndpoint } from "./model-endpoint-state";
import { ImageEndpointFields } from "./image-endpoint-fields";
import type { ImageEndpointProbeReport, ModelEndpointApiKey } from "../../../api/contracts";
import "./model-endpoint-settings.css";

type Props = { kind: ModelEndpointKind; config: AppConfig; secretSentinel: string; onChange: (config: AppConfig) => void };

/**
 * 【模型接入】【独立入口】编辑生图或 JEV 请求地址、密钥和模型标识。
 * @param props 接入类型、配置、密钥保留标记和修改回调
 * @returns 复用设置列表与表单样式的接入页面
 */
export function ModelEndpointSettings({ kind, config, secretSentinel, onChange }: Props) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const items = (config.model_endpoints ?? []).filter((item) => item.kind === kind);
  const [selectedId, setSelectedId] = useState("");
  const [fetching, setFetching] = useState(false);
  const [probing, setProbing] = useState(false);
  const [models, setModels] = useState<string[]>([]);
  const [probe, setProbe] = useState<ImageEndpointProbeReport | null>(null);
  const [fetchError, setFetchError] = useState("");
  const [probeError, setProbeError] = useState("");
  const selected = items.find((item) => item.id === selectedId) ?? items[0];
  const title = kind === "jev" ? t("JEV models", "JEV 模型") : t("Image models", "生图模型");
  const hidden = Boolean(secretSentinel) && selected?.api_key === secretSentinel;

  const endpointKeys = selected?.api_keys?.length
    ? selected.api_keys
    : selected?.api_key
      ? [{ id: "key-1", api_key: selected.api_key, label: "" }]
      : [];
  const selectedKey = selected?.api_key_selected ?? endpointKeys[0]?.id;

  useEffect(() => {
    if (selected && selected.id !== selectedId) setSelectedId(selected.id);
  }, [selected?.id, selectedId]);

  useEffect(() => {
    setModels(selected?.models ?? []);
    setProbe(null);
    setFetchError("");
    setProbeError("");
  }, [selected?.id]);

  /** 【模型接入】【新增】追加当前类型的独立配置；无参数，无返回值 */
  const add = () => {
    const item = newModelEndpoint(config.model_endpoints ?? [], kind, title);
    onChange({ ...config, model_endpoints: [...(config.model_endpoints ?? []), item] });
    setSelectedId(item.id);
  };

  /** 【模型接入】【编辑】更新当前项；参数为字段补丁，返回无 */
  const patch = (value: Partial<Omit<ModelEndpointConfig, "id" | "kind">>) => {
    if (selected) onChange(updateModelEndpoint(config, selected.id, value));
  };

  /** 获取当前生图端点模型目录并同步到配置草稿。 */
  const fetchModels = async () => {
    if (!selected) return;
    setFetching(true);
    setFetchError("");
    try {
      const response = await api.imageModels.models(selected);
      setModels(response.models);
      patch({ models: response.models, model: selected.model || response.models[0] || "" });
    } catch (error) {
      setFetchError(toDisplayError(error, "Failed to fetch image models", "获取生图模型失败").message);
    } finally {
      setFetching(false);
    }
  };

  /** 对当前生图端点发起真实小尺寸探测。 */
  const probeEndpoint = async () => {
    if (!selected) return;
    setProbing(true);
    setProbeError("");
    try {
      setProbe(await api.imageModels.test(selected));
    } catch (error) {
      setProbeError(toDisplayError(error, "Image endpoint test failed", "生图端点测试失败").message);
    } finally {
      setProbing(false);
    }
  };

  /** 读取专用端点指定密钥的真实值，用于哨兵密钥的按需查看。 */
  const revealKey = (keyId: string) => api.config.modelEndpointSecret(selected?.id ?? "", keyId).then((response) => response.api_key);

  /** 【模型接入】【删除】使用统一对话框确认删除当前配置；无参数，返回完成通知 */
  const remove = async () => {
    if (!selected) return;
    const id = selected.id;
    if (!await confirm({ title: t("Delete model connection", "删除模型接入"), description: t(`Delete “${selected.name}”?`, `删除“${selected.name}”的接入配置？`), confirmLabel: t("Delete", "删除"), danger: true })) return;
    onChange({ ...config, model_endpoints: (config.model_endpoints ?? []).filter((item) => item.id !== id) });
  };

  return (
    <div className="settings-objects-layout">
      <ObjectListPanel title={title} items={items.map((item) => ({ id: item.id, name: item.name, meta: item.model }))} selectedId={selected?.id ?? ""} searchPlaceholder={t("Search models", "搜索模型")} addLabel={t("Add model connection", "新增模型接入")} onSelect={setSelectedId} onAdd={add} />
      <div className="settings-editor min-w-0">
        <EditorHeader kicker={title} title={selected?.name ?? title} description={t("Save connection settings for later use.", "保存模型接入信息，供后续功能使用。")} actions={selected && <Button className="settings-secondary danger" onClick={() => void remove()}><Trash2 size={14} aria-hidden />{t("Delete", "删除")}</Button>} />
        {selected
          ? kind === "image_generation"
            ? <ImageEndpointFields
              endpoint={selected}
              keys={endpointKeys as ModelEndpointApiKey[]}
              selectedKey={selectedKey}
              secretSentinel={secretSentinel}
              models={models}
              fetching={fetching}
              probing={probing}
              probe={probe}
              fetchError={fetchError}
              probeError={probeError}
              onPatch={patch}
              onKeysChange={(value) => patch({ ...value, api_key: "" })}
              onRevealKey={revealKey}
              onFetchModels={() => void fetchModels()}
              onProbe={() => void probeEndpoint()}
            />
            : <SettingsGroup title={t("Connection", "连接配置")}>
              <div className="grid min-w-0 grid-cols-1 gap-4 md:grid-cols-2">
                <TextFieldRow label={t("Name", "名称")} value={selected.name} onChange={(name) => patch({ name })} />
                <TextFieldRow label={t("Model ID", "模型标识")} value={selected.model} onChange={(model) => patch({ model })} hint={t("Use the model ID required by this endpoint; leave blank if none is needed.", "填写该接口要求的模型标识；接口不需要时可留空。")} />
                <div className="min-w-0 md:col-span-2"><TextFieldRow label={t("Request endpoint", "请求地址")} value={selected.endpoint} placeholder="https://example.com:8443/api/model" onChange={(endpoint) => patch({ endpoint })} hint={t("Complete HTTP(S) request URL, including any port and path.", "完整 HTTP(S) 请求地址，包含所需端口和路径。")} /></div>
                <div className="settings-field min-w-0 md:col-span-2"><label htmlFor={`model-endpoint-key-${selected.id}`}>API Key</label><PasswordField key={selected.id} id={`model-endpoint-key-${selected.id}`} ariaLabel={t("API key", "API Key")} value={hidden ? "" : selected.api_key} savedValueHint={hidden ? t("Saved", "已保存") : undefined} placeholder={hidden ? t("Enter a new key to replace it", "输入新 Key 以替换") : "$env:MODEL_API_KEY"} onClearSavedValue={hidden ? () => patch({ api_key: "" }) : undefined} onChange={(api_key) => patch({ api_key })} /><small>{t("Stored independently from LLM provider keys. Supports $env:VARIABLE references.", "与 LLM 供应商的 Key 独立保存，支持 $env:变量名 引用。")}</small></div>
              </div>
            </SettingsGroup>
          : <div className="settings-empty"><p>{t("No model connections configured.", "尚未配置模型接入。")}</p><Button className="settings-secondary" onClick={add}>{t("Add model connection", "新增模型接入")}</Button></div>}
      </div>
    </div>
  );
}
