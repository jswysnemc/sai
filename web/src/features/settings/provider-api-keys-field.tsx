import { Plus, Trash2 } from "lucide-react";
import { api } from "../../api/client";
import type { ProviderApiKey } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { PasswordField } from "../../shared/ui/password-field";
import { useI18n } from "../i18n/use-i18n";
import "./provider-api-keys-field.css";

type ProviderApiKeysFieldProps = {
  providerId: string;
  keys: ProviderApiKey[];
  selected?: string;
  balance: boolean;
  secretSentinel: string;
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
 * 渲染供应商的多密钥列表、负载均衡开关与选中项。
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
  onChange
}: ProviderApiKeysFieldProps) {
  const { t } = useI18n();

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

  /** 追加一个空密钥并选中它。 */
  const addKey = () => {
    const id = nextKeyId(keys);
    onChange({ api_keys: [...keys, { id, api_key: "", label: "" }], api_key_selected: id, api_key_balance: balance });
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
    updateKeys(keys.map((key) => (key.id === id ? { ...key, ...patch } : key)));

  return (
    <div className="provider-api-keys-field">
      <div className="provider-api-keys-head">
        <span>{t("API keys", "接口密钥")}</span>
        <Button className="provider-api-keys-add" onClick={addKey}>
          <Plus size={13} />
          {t("Add key", "新增密钥")}
        </Button>
      </div>
      {keys.length === 0 && (
        <p className="provider-api-keys-empty">
          {t("No extra keys. The single API Key above is used.", "暂无额外密钥，使用上方的单个 API Key。")}
        </p>
      )}
      <ul className="provider-api-keys-list">
        {keys.map((key) => (
          <li className="provider-api-key-row" key={key.id}>
            <div className="provider-api-key-value">
              <PasswordField
                value={key.api_key}
                onReveal={secretSentinel.length > 0 && key.api_key === secretSentinel
                  ? () => api.config.providerSecret(providerId, key.id).then((response) => response.api_key)
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
            <button
              type="button"
              className="provider-api-key-remove"
              onClick={() => removeKey(key.id)}
              aria-label={t("Remove key", "移除密钥")}
              title={t("Remove key", "移除密钥")}
            >
              <Trash2 size={13} />
            </button>
          </li>
        ))}
      </ul>
      {keys.length > 1 && (
        <div className="provider-api-keys-controls">
          <label className="provider-api-keys-balance">
            <input
              type="checkbox"
              checked={balance}
              onChange={(event) => onChange({ api_keys: keys, api_key_selected: selected, api_key_balance: event.target.checked })}
            />
            <span>
              <strong>{t("Load balance across keys", "在密钥间负载均衡")}</strong>
              <small>{t("Round-robin a different key on each request", "每次请求轮换使用不同密钥")}</small>
            </span>
          </label>
          {!balance && (
            <label className="provider-api-keys-selected">
              <span>{t("Active key", "当前选用")}</span>
              <select
                value={selected ?? ""}
                onChange={(event) => onChange({ api_keys: keys, api_key_selected: event.target.value, api_key_balance: balance })}
              >
                {keys.map((key, index) => (
                  <option value={key.id} key={key.id}>
                    {key.label?.trim() || `${t("Key", "密钥")} ${index + 1}`}
                  </option>
                ))}
              </select>
            </label>
          )}
        </div>
      )}
    </div>
  );
}
