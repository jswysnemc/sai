import { Trash2 } from "../../../shared/ui/icons";
import { api } from "../../../api/client";
import type { AppConfig, ModelEndpointConfig, ModelEndpointKind } from "../../../api/contracts/config";
import { Button } from "../../../shared/ui/button/button";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { useI18n } from "../../i18n/use-i18n";
import { DetailHeader, EmptyGuide, MasterDetail, ObjectList } from "../kit";
import { useSettingsItem } from "../shell/use-settings-item";
import { newModelEndpoint, updateModelEndpoint } from "./model-endpoint-state";
import { ImageEndpointFields } from "./image-endpoint-fields";
import { JevEndpointFields } from "./jev-endpoint-fields";
import { releaseJevEndpoint } from "../jev/jev-config";
import type { ModelEndpointApiKey } from "../../../api/contracts";
import "./model-endpoint-settings.css";

type Props = { kind: ModelEndpointKind; config: AppConfig; secretSentinel: string; onChange: (config: AppConfig) => void };

/**
 * 【模型接入】【独立入口】编辑生图或 Jev 请求地址、密钥和模型标识。
 * @param props 接入类型、配置、密钥保留标记和修改回调
 * @returns 复用设置列表与表单样式的接入页面
 */
export function ModelEndpointSettings({ kind, config, secretSentinel, onChange }: Props) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const items = (config.model_endpoints ?? []).filter((item) => item.kind === kind);
  const [selectedId, setSelectedId] = useSettingsItem(items.map((item) => item.id));
  const selected = items.find((item) => item.id === selectedId) ?? items[0];
  const title = kind === "jev" ? t("Jev connections", "Jev 接入") : t("Image models", "生图模型");

  const endpointKeys = selected?.api_keys?.length
    ? selected.api_keys
    : selected?.api_key
      ? [{ id: "key-1", api_key: selected.api_key, label: "" }]
      : [];
  const selectedKey = selected?.api_key_selected ?? endpointKeys[0]?.id;

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

  /** 读取专用端点指定密钥的真实值，用于哨兵密钥的按需查看。 */
  const revealKey = (keyId: string) => api.config.modelEndpointSecret(selected?.id ?? "", keyId).then((response) => response.api_key);

  /** 【模型接入】【删除】使用统一对话框确认删除当前配置；无参数，返回完成通知 */
  const remove = async () => {
    if (!selected) return;
    const id = selected.id;
    if (!await confirm({ title: t("Delete model connection", "删除模型接入"), description: t(`Delete “${selected.name}”?`, `删除“${selected.name}”的接入配置？`), confirmLabel: t("Delete", "删除"), danger: true })) return;
    onChange(releaseJevEndpoint({ ...config, model_endpoints: (config.model_endpoints ?? []).filter((item) => item.id !== id) }, id));
  };

  if (!items.length) return <EmptyGuide title={t("No model connections configured", "尚未配置模型接入")} description={t("Add an endpoint to configure its address, credentials and model.", "新增接入后配置请求地址、凭据与模型。")} action={<Button onClick={add}>{t("Add model connection", "新增模型接入")}</Button>} />;

  const detail = <>
    {selected && <DetailHeader title={selected.name} actions={<>{items.length === 1 && <Button size="small" onClick={add}>{t("Add model connection", "新增模型接入")}</Button>}<Button variant="ghost" size="small" onClick={() => void remove()}><Trash2 size={14} aria-hidden />{t("Delete", "删除")}</Button></>} />}
    {selected
      ? kind === "image_generation"
        ? <ImageEndpointFields
          endpoint={selected}
          keys={endpointKeys as ModelEndpointApiKey[]}
          selectedKey={selectedKey}
          secretSentinel={secretSentinel}
          onPatch={patch}
          onKeysChange={(value) => patch({ ...value, api_key: "" })}
          onRevealKey={revealKey}
        />
        : <JevEndpointFields
          endpoint={selected}
          keys={endpointKeys as ModelEndpointApiKey[]}
          selectedKey={selectedKey}
          secretSentinel={secretSentinel}
          onPatch={patch}
          onKeysChange={(value) => patch({ ...value, api_key: "" })}
          onRevealKey={revealKey}
        />
      : <EmptyGuide title={t("No model connections configured", "尚未配置模型接入")} description={t("Add an endpoint to configure its address, credentials and model.", "新增接入后配置请求地址、凭据与模型。")} action={<Button onClick={add}>{t("Add model connection", "新增模型接入")}</Button>} />}
  </>;
  return items.length === 1 ? <div className="min-w-0">{detail}</div> : <MasterDetail list={<ObjectList title={t("Connections", "接入列表")} items={items.map((item) => ({ id: item.id, name: item.name, meta: item.model }))} selectedId={selected?.id ?? ""} searchPlaceholder={t("Search models", "搜索模型")} addLabel={t("Add model connection", "新增模型接入")} onSelect={setSelectedId} onAdd={add} />}>{detail}</MasterDetail>;
}
