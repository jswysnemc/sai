import type { ModelEndpointApiKey, ModelEndpointConfig } from "../../../api/contracts";

/**
 * 构造只使用当前选中密钥的探测草稿。
 *
 * 旧配置只存了单值密钥时，把占位符留在 `api_key` 上，服务端才能换回真实值。
 *
 * @param endpoint 当前接入草稿
 * @param keys 编辑器中的密钥列表
 * @param selectedKey 当前选中的密钥标识
 * @param secretSentinel 已保存密钥的占位符
 * @returns 发给测试或拉目录接口的接入
 */
export function endpointForProbe(
  endpoint: ModelEndpointConfig,
  keys: ModelEndpointApiKey[],
  selectedKey: string | undefined,
  secretSentinel: string
): ModelEndpointConfig {
  const selected = keys.find((key) => key.id === selectedKey) ?? keys[0];
  if (selected && secretSentinel && selected.api_key === secretSentinel && keys.length <= 1) {
    return { ...endpoint, api_key: secretSentinel, api_keys: [], api_key_balance: false };
  }
  if (!selected) return { ...endpoint, api_key_balance: false };
  return {
    ...endpoint,
    api_key: "",
    api_keys: [selected],
    api_key_selected: selected.id,
    api_key_balance: false
  };
}
