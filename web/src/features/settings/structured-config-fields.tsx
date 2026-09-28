import { useI18n } from "../i18n/use-i18n";
import { mergeSecretValues } from "./controls/merge-secret-values";
import { ChoicePills, FieldGrid, SettingsField, SettingsPanel, SkNumberInput, SkSecretInput, SkTextArea, SkTextInput, SwitchField } from "./kit";
import { fieldLabel, fieldSelectOptions, isSecretField } from "./structured-field-metadata";

export { fieldSelectOptions } from "./structured-field-metadata";

type StructuredConfigFieldsProps = {
  value: Record<string, unknown>;
  secretSentinel?: string;
  /** 完整配置路径，仅用于提示和字段元数据 */
  configPath?: string;
  /** 字段搜索使用的锚点前缀 */
  anchorPrefix?: string;
  onChange: (value: Record<string, unknown>) => void;
};

/**
 * 【Web 设置】【结构化配置】按值类型组合统一字段控件，保留未知字段和脱敏值。
 * @param props 配置对象、脱敏标记、配置路径、锚点前缀与更新回调
 * @returns 可编辑字段栅格
 */
export function StructuredConfigFields({ value, secretSentinel = "", configPath, anchorPrefix, onChange }: StructuredConfigFieldsProps) {
  const { t } = useI18n();
  const entries = Object.entries(value);
  if (!entries.length) return <p className="sk-field-hint">{t("No editable fields in this group.", "当前配置组没有可编辑字段。")}</p>;
  return (
    <FieldGrid>
      {entries.map(([key, field]) => (
        <StructuredField key={key} name={key} value={field} secretSentinel={secretSentinel}
          configKey={configPath ? `${configPath}.${key}` : key}
          anchor={anchorPrefix ? `${anchorPrefix}.${key}` : undefined}
          onChange={(next) => onChange({ ...value, [key]: next })} />
      ))}
    </FieldGrid>
  );
}

/**
 * 【Web 设置】【结构化字段】为标量、数组及嵌套对象选择控件。
 * @param props 字段名称、值、路径、锚点、脱敏标记与更新回调
 * @returns 单个字段或嵌套面板
 */
function StructuredField({ name, value, configKey, anchor, secretSentinel, onChange }: {
  name: string;
  value: unknown;
  configKey: string;
  anchor?: string;
  secretSentinel: string;
  onChange: (value: unknown) => void;
}) {
  const { t } = useI18n();
  const label = fieldLabel(name, t);
  const field = { label, configKey, anchor };
  if (typeof value === "boolean") return <SwitchField {...field} checked={value} onChange={onChange} />;
  if (typeof value === "number") return <SettingsField {...field} size="sm"><SkNumberInput value={value} onChange={onChange} /></SettingsField>;
  const options = typeof value === "string" ? fieldSelectOptions(name, t) : null;
  if (options) return <SettingsField {...field}><ChoicePills value={String(value)} options={options} onChange={onChange} /></SettingsField>;
  if (Array.isArray(value)) {
    const secret = isSecretField(name);
    const current = value.map((item) => String(item ?? ""));
    const hiddenCount = secret && secretSentinel ? current.filter((item) => item === secretSentinel).length : 0;
    const visible = secret && secretSentinel ? current.filter((item) => item !== secretSentinel && item.trim()) : current;
    return (
      <SettingsField {...field} span="full" hint={hiddenCount ? t(`${hiddenCount} saved secrets remain unchanged; add one item per line.`, `已保存的 ${hiddenCount} 个敏感值保持不变；每行新增一项。`) : t("One item per line.", "每行填写一项。") }>
        <SkTextArea rows={Math.min(8, Math.max(3, visible.length + 1))} value={visible.join("\n")} onChange={(text) => {
          // 1. 合并可见条目并保留服务器脱敏值
          const next = text.split("\n").map((item) => item.trim()).filter(Boolean);
          onChange(secret && secretSentinel ? mergeSecretValues(current, next, secretSentinel) : next);
        }} />
      </SettingsField>
    );
  }
  if (value && typeof value === "object") return (
    <SettingsPanel title={label} className="col-span-full">
      <StructuredConfigFields value={value as Record<string, unknown>} configPath={configKey} anchorPrefix={anchor} secretSentinel={secretSentinel} onChange={onChange} />
    </SettingsPanel>
  );
  return (
    <SettingsField {...field}>
      {isSecretField(name)
        ? <SkSecretInput value={String(value ?? "")} secretSentinel={secretSentinel} onChange={onChange} />
        : <SkTextInput value={String(value ?? "")} onChange={onChange} />}
    </SettingsField>
  );
}
