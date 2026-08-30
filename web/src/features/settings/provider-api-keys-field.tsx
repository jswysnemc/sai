import { Plus, Trash2 } from "lucide-react";
import { api } from "../../api/client";
import type { ProviderApiKey } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { PasswordField } from "../../shared/ui/password-field";
import { Select } from "../../shared/ui/select/select";
import { useI18n } from "../i18n/use-i18n";
import "./provider-api-keys-field.css";

type ProviderApiKeysFieldProps = {
  providerId: string;
  keys: ProviderApiKey[];
  selected?: string;
  balance: boolean;
  secretSentinel: string;
  onRevealKey?: (keyId: string) => Promise<string>;
  onChange: (patch: {
    api_keys: ProviderApiKey[];
    api_key_selected?: string;
    api_key_balance: boolean;
  }) => void;
};

/**
 * 生成多密钥列表里尚未占用的稳定标识。
 *
 * @param keys 现有多密钥列表
 * @returns 形如 key-N 的新标识
 */
function nextKeyId(keys: ProviderApiKey[]): string {
  let suffix = 1;
  while (keys.some((key) => key.id === `key-${suffix}`)) suffix += 1;
  return `key-${suffix}`;
}

/**
 * 编辑器始终至少展示一个空密钥框，避免用户先点「新增」才能填写。
 *
 * @param keys 已保存的密钥列表
 * @returns 用于渲染的密钥列表
 */
export function editorProviderApiKeys(keys: ProviderApiKey[]): ProviderApiKey[] {
  if (keys.length > 0) return keys;
  return [{ id: "key-1", api_key: "", label: "" }];
}

/**
 * 渲染供应商的多密钥列表、使用策略与工作或测试密钥。
 *
 * 每个密钥带稳定标识，服务端据此在脱敏回填时按 id 对齐，
 * 删除或重排后不会串用密钥。
 *
 * @param props 供应商标识、密钥列表、选中项、负载均衡开关与更新回调
 * @returns 多密钥编辑区
 */
export function ProviderApiKeysField({
  providerId,
  keys,
  selected,
  balance,
  secretSentinel,
  onRevealKey,
  onChange
}: ProviderApiKeysFieldProps) {
  const { t } = useI18n();
  const editorKeys = editorProviderApiKeys(keys);
  const selectedId = selected && editorKeys.some((key) => key.id === selected)
    ? selected
    : editorKeys[0]?.id;

  /**
   * 以新列表更新，并在选中项被删除时回落到首个。
   *
   * @param next 更新后的密钥列表
   * @returns 无返回值
   */
  const updateKeys = (next: ProviderApiKey[]) => {
    const nextSelected = selected && next.some((key) => key.id === selected)
      ? selected
      : next[0]?.id;
    onChange({ api_keys: next, api_key_selected: nextSelected, api_key_balance: balance });
  };

  /**
   * 追加一个空密钥并选中它。
   *
   * @returns 无返回值
   */
  const addKey = () => {
    const id = nextKeyId(editorKeys);
    onChange({
      api_keys: [...editorKeys, { id, api_key: "", label: "" }],
      api_key_selected: id,
      api_key_balance: balance
    });
  };

  /**
   * 删除指定密钥。
   *
   * @param id 密钥标识
   * @returns 无返回值
   */
  const removeKey = (id: string) => updateKeys(keys.filter((key) => key.id !== id));

  /**
   * 更新指定密钥的某个字段。
   *
   * @param id 密钥标识
   * @param patch 字段局部更新
   * @returns 无返回值
   */
  const updateKey = (id: string, patch: Partial<ProviderApiKey>) =>
    updateKeys(editorKeys.map((key) => (key.id === id ? { ...key, ...patch } : key)));

  return (
    <div className="provider-api-keys-field">
      <div className="provider-api-keys-head">
        <span>{t("API keys", "接口密钥")}</span>
        <Button className="provider-api-keys-add" onClick={addKey}>
          <Plus size={13} />
          {t("Add key", "新增密钥")}
        </Button>
      </div>
      <ul className="provider-api-keys-list">
        {editorKeys.map((key, index) => (
          // 同一供应商内 key.id 唯一，跨供应商会重复：带上供应商标识，
          // 避免切换供应商时 React 复用上一个供应商的输入框实例
          <li className="provider-api-key-row" key={`${providerId}:${key.id}`}>
            <div className="provider-api-key-value">
              <PasswordField
                value={key.api_key}
                placeholder={t(`API key ${index + 1}`, `接口密钥 ${index + 1}`)}
                onReveal={secretSentinel.length > 0 && key.api_key === secretSentinel
                  ? () => onRevealKey
                    ? onRevealKey(key.id)
                    : api.config.providerSecret(providerId, key.id).then((response) => response.api_key)
                  : undefined}
                onChange={(value) => updateKey(key.id, { api_key: value })}
              />
            </div>
            <input
              className="provider-api-key-label"
              value={key.label ?? ""}
              placeholder={t("Note", "备注")}
              spellCheck={false}
              onChange={(event) => updateKey(key.id, { label: event.target.value })}
            />
            {editorKeys.length > 1 && (
              <button
                type="button"
                className="provider-api-key-remove"
                onClick={() => removeKey(key.id)}
                aria-label={t("Remove key", "移除密钥")}
                title={t("Remove key", "移除密钥")}
              >
                <Trash2 size={13} />
              </button>
            )}
          </li>
        ))}
      </ul>
      {editorKeys.length > 1 && (
        <div className="provider-api-keys-controls">
          <div className="provider-api-keys-strategy">
            <span>{t("Key usage", "密钥使用方式")}</span>
            <Select
              value={balance ? "balance" : "selected"}
              options={[
                {
                  value: "selected",
                  label: t("Use selected key", "固定使用所选密钥"),
                  description: t("Send every request with one key", "所有请求使用同一个密钥")
                },
                {
                  value: "balance",
                  label: t("Load balance", "负载均衡"),
                  description: t("Rotate across all configured keys", "在全部已配置密钥间轮换")
                }
              ]}
              onChange={(value) => onChange({
                api_keys: editorKeys,
                api_key_selected: selectedId,
                api_key_balance: value === "balance"
              })}
              ariaLabel={t("Key usage", "密钥使用方式")}
            />
          </div>
          <div className="provider-api-keys-selected">
            <span>{balance ? t("Test key", "测试密钥") : t("Working key", "工作密钥")}</span>
            <Select
              value={selectedId ?? ""}
              options={editorKeys.map((key, index) => ({
                value: key.id,
                label: key.label?.trim() || `${t("Key", "密钥")} ${index + 1}`
              }))}
              onChange={(value) => onChange({ api_keys: editorKeys, api_key_selected: value, api_key_balance: balance })}
              ariaLabel={balance ? t("API key used for tests", "用于测试的接口密钥") : t("Working API key", "工作接口密钥")}
              menuMinimumWidth={180}
            />
            {balance && (
              <small>
                {t("Requests rotate across all keys; tests use this key.", "正式请求会在全部密钥间轮换；连通性测试使用这里选中的密钥。")}
              </small>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
