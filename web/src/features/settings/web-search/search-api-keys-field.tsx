import { Plus, Trash2 } from "../../../shared/ui/icons";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { SettingsField, SkSecretInput } from "../kit";
import { fieldAnchorId } from "../search/field-anchor";

type SearchApiKeysFieldProps = {
  anchor?: string;
  keys: string[];
  environmentVariable: string;
  secretSentinel: string;
  onChange: (keys: string[]) => void;
};

/**
 * 【网页搜索】【凭据编辑】使用独立密钥行，保留隐藏密钥在服务端数组中的位置。
 * @param props 密钥列表、环境变量名、脱敏标记与更新回调
 * @returns 具备显隐、已保存标记和清除操作的密钥列表
 */
export function SearchApiKeysField({ anchor, keys, environmentVariable, secretSentinel, onChange }: SearchApiKeysFieldProps) {
  const { t } = useI18n();
  const rows = keys.length ? keys : [""];
  /** 修改单个密钥槽位，清除时保留空槽，避免后续隐藏密钥错位；参数为索引与值，无返回值 */
  const update = (index: number, value: string) => onChange(rows.map((key, position) => position === index ? value : key));
  return <div className="col-span-full grid min-w-0 gap-2" id={anchor ? fieldAnchorId(anchor) : undefined}>
    <div className="flex flex-wrap items-center justify-between gap-2 text-xs">
      <span>{t("API keys", "接口密钥")}</span>
      <Button size="small" onClick={() => onChange([...rows, ""])}><Plus size={14} />{t("Add key", "添加密钥")}</Button>
    </div>
    {rows.map((key, index) => <SettingsField key={index} label={t(`API key ${index + 1}`, `接口密钥 ${index + 1}`)}>
      <div className="flex min-w-0 items-center gap-1">
        <div className="min-w-0 flex-1"><SkSecretInput value={key} secretSentinel={secretSentinel} onChange={(value) => update(index, value)} placeholder={`$env:${environmentVariable}`} /></div>
        <Button variant="ghost-danger" size="icon" disabled={!key} aria-label={t(`Clear key ${index + 1}`, `清除密钥 ${index + 1}`)} onClick={() => update(index, "")}><Trash2 size={14} /></Button>
      </div>
    </SettingsField>)}
    <p className="sk-field-hint m-0">{t(`One key or environment reference per row. Also reads ${environmentVariable}.`, `每行填写一个密钥或环境变量引用，也会读取 ${environmentVariable}。`)}</p>
  </div>;
}
