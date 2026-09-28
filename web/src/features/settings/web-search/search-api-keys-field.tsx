import { SettingsField, SkListInput } from "../kit";
import { Trash2 } from "../../../shared/ui/icons";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import {
  hiddenSearchApiKeyCount,
  mergeSearchApiKeyText,
  visibleSearchApiKeys
} from "./web-search-config";

type SearchApiKeysFieldProps = {
  anchor?: string;
  keys: string[];
  environmentVariable: string;
  secretSentinel: string;
  onChange: (keys: string[]) => void;
};

/**
 * 渲染搜索供应商密钥列表，并隐藏服务端敏感字段占位符。
 *
 * @param props 密钥列表、环境变量名称、占位符和更新回调
 * @returns 多行密钥编辑字段
 */
export function SearchApiKeysField({
  anchor,
  keys,
  environmentVariable,
  secretSentinel,
  onChange
}: SearchApiKeysFieldProps) {
  const { t } = useI18n();
  const visibleKeys = visibleSearchApiKeys(keys, secretSentinel);
  const hiddenCount = hiddenSearchApiKeyCount(keys, secretSentinel);

  /**
   * 清除服务端已保存的隐藏密钥，保留当前可见条目。
   *
   * @returns 无返回值
   */
  const clearSavedKeys = (): void => {
    onChange(visibleKeys.map((key) => key.trim()).filter(Boolean));
  };

  return (
    <SettingsField label={t("API keys", "接口密钥")} anchor={anchor} span="full" aside={hiddenCount > 0 && <Button variant="ghost-danger" size="small" onClick={clearSavedKeys}><Trash2 size={14} />{t("Clear saved keys", "清除已保存密钥")}</Button>} hint={hiddenCount > 0 ? t(`${hiddenCount} saved keys remain hidden; add one key or environment reference per line.`, `已隐藏 ${hiddenCount} 个已保存密钥；每行新增一个密钥或环境变量引用。`) : t(`One key or environment reference per line. Also reads ${environmentVariable}.`, `每行填写一个密钥或环境变量引用，也会读取 ${environmentVariable}。`)}>
      <SkListInput value={visibleKeys} rows={Math.min(7, Math.max(3, visibleKeys.length + 1))} placeholder={"$env:" + environmentVariable} onChange={(value) => onChange(mergeSearchApiKeyText(keys, value.join("\n"), secretSentinel))} />
    </SettingsField>
  );
}
