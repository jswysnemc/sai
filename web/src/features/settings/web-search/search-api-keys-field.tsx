import { Plus, Trash2 } from "../../../shared/ui/icons";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { SkSecretInput } from "../kit";
import { fieldAnchorId } from "../search/field-anchor";

type SearchApiKeysFieldProps = {
  anchor?: string;
  keys: string[];
  environmentVariable: string;
  secretSentinel: string;
  onChange: (keys: string[]) => void;
};

/**
 * 删除指定密钥行；至少保留一个空槽以便继续填写。
 *
 * @param keys 当前密钥列表
 * @param index 要删除的下标
 * @returns 删除后的密钥列表
 */
export function removeSearchApiKey(keys: string[], index: number): string[] {
  const rows = keys.length ? keys : [""];
  const next = rows.filter((_, position) => position !== index);
  return next.length ? next : [""];
}

/**
 * 【网页搜索】【凭据编辑】使用独立密钥行，保留隐藏密钥在服务端数组中的位置。
 *
 * @param props 密钥列表、环境变量名、脱敏标记与更新回调
 * @returns 具备显隐、已保存标记和删除操作的密钥列表
 */
export function SearchApiKeysField({ anchor, keys, environmentVariable, secretSentinel, onChange }: SearchApiKeysFieldProps) {
  const { t } = useI18n();
  const rows = keys.length ? keys : [""];
  /**
   * 修改单个密钥槽位，清除时保留空槽，避免后续隐藏密钥错位。
   *
   * @param index 槽位下标
   * @param value 新密钥
   * @returns 无返回值
   */
  const update = (index: number, value: string) => onChange(rows.map((key, position) => (position === index ? value : key)));
  return (
    <div className="search-api-keys" id={anchor ? fieldAnchorId(anchor) : undefined}>
      <div className="search-api-keys-head">
        <span>{t("API keys", "接口密钥")}</span>
        <Button size="small" onClick={() => onChange([...rows, ""])}>
          <Plus size={14} />
          {t("Add key", "添加密钥")}
        </Button>
      </div>
      <ul className="search-api-keys-list">
        {rows.map((key, index) => (
          <li className="search-api-key-row" key={index}>
            <SkSecretInput
              value={key}
              secretSentinel={secretSentinel}
              onChange={(value) => update(index, value)}
              placeholder={`$env:${environmentVariable}`}
              ariaLabel={t(`API key ${index + 1}`, `接口密钥 ${index + 1}`)}
            />
            <Button
              variant="ghost-danger"
              size="icon"
              disabled={rows.length === 1 && !key}
              aria-label={t(`Remove key ${index + 1}`, `删除密钥 ${index + 1}`)}
              onClick={() => onChange(removeSearchApiKey(rows, index))}
            >
              <Trash2 size={14} />
            </Button>
          </li>
        ))}
      </ul>
      <p className="search-api-keys-hint">
        {t(
          `One key or environment reference per row. Also reads ${environmentVariable}.`,
          `每行填写一个密钥或环境变量引用，也会读取 ${environmentVariable}。`
        )}
      </p>
    </div>
  );
}
