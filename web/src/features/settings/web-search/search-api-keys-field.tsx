import { Trash2 } from "lucide-react";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import {
  hiddenSearchApiKeyCount,
  mergeSearchApiKeyText,
  visibleSearchApiKeys
} from "./web-search-config";

type SearchApiKeysFieldProps = {
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
    <div className="settings-field full search-api-keys-field">
      <div className="search-api-keys-head">
        <span>{t("API keys", "接口密钥")}</span>
        {hiddenCount > 0 && (
          <Button variant="ghost-danger" onClick={clearSavedKeys}>
            <Trash2 size={13} />
            {t("Clear saved keys", "清除已保存密钥")}
          </Button>
        )}
      </div>
      <textarea
        rows={Math.min(7, Math.max(3, visibleKeys.length + 1))}
        value={visibleKeys.join("\n")}
        placeholder={"$env:" + environmentVariable}
        onChange={(event) => onChange(mergeSearchApiKeyText(keys, event.target.value, secretSentinel))}
        spellCheck={false}
        autoComplete="off"
      />
      <small>
        {hiddenCount > 0
          ? t(
              hiddenCount === 1
                ? "1 saved key remains hidden. Add direct keys or $env references one per line."
                : hiddenCount + " saved keys remain hidden. Add direct keys or $env references one per line.",
              "已隐藏 " + hiddenCount + " 个已保存密钥。每行可新增一个直接密钥或 $env 引用。"
            )
          : t(
              "One key or $env reference per line. The runtime also checks " + environmentVariable + ".",
              "每行填写一个密钥或 $env 引用。运行时也会读取 " + environmentVariable + "。"
            )}
      </small>
    </div>
  );
}
