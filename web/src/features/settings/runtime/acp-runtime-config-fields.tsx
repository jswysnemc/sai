import type { EngineStatusResponse } from "../../../api/contracts";
import { Select } from "../../../shared/ui/select/select";
import { useI18n } from "../../i18n/use-i18n";
import { StructuredConfigFields } from "../structured-config-fields";

type RuntimeValue = string | boolean;

export type AcpRuntimeOption = {
  id: string;
  name: string;
  description: string;
  category?: string;
  type: "select" | "boolean";
  currentValue: RuntimeValue;
  values: Array<{ value: string; label: string; description?: string }>;
};

type AcpRuntimeConfigFieldsProps = {
  acp: Record<string, unknown>;
  runtime: EngineStatusResponse["acp_runtime"];
  onChange: (patch: Record<string, unknown>) => void;
};

/**
 * 将 ACP 运行状态解析成可渲染配置项。
 *
 * @param input agent 公布的标准 configOptions
 * @returns 已过滤的选择项和布尔项
 */
export function parseAcpRuntimeOptions(input: unknown): AcpRuntimeOption[] {
  if (!Array.isArray(input)) return [];
  return input.flatMap<AcpRuntimeOption>((candidate): AcpRuntimeOption[] => {
    if (!candidate || typeof candidate !== "object") return [];
    const option = candidate as Record<string, unknown>;
    if (typeof option.id !== "string" || typeof option.name !== "string") return [];
    const category = typeof option.category === "string" ? option.category : undefined;
    const description = typeof option.description === "string" ? option.description : "";
    if (option.type === "boolean" && typeof option.currentValue === "boolean") {
      return [{
        id: option.id,
        name: option.name,
        description,
        category,
        type: "boolean" as const,
        currentValue: option.currentValue,
        values: []
      }];
    }
    if (option.type !== "select" || typeof option.currentValue !== "string") return [];
    const values = flattenSelectOptions(option.options);
    if (values.length === 0) return [];
    return [{
      id: option.id,
      name: option.name,
      description,
      category,
      type: "select" as const,
      currentValue: option.currentValue,
      values
    }];
  });
}

/**
 * 渲染 ACP agent 实际公布的会话配置项。
 *
 * @param props ACP 配置、运行状态与更新回调
 * @returns 标准类别和扩展配置控件
 */
export function AcpRuntimeConfigFields({ acp, runtime, onChange }: AcpRuntimeConfigFieldsProps) {
  const { t } = useI18n();
  const options = parseAcpRuntimeOptions(runtime?.config_options);
  const authMethods = parseAuthMethods(runtime?.auth_methods);
  const configuredOptions = (acp.config_options as Record<string, unknown> | undefined) ?? {};
  const represented = new Set(options.map((option) => option.id));
  const unmatched = Object.fromEntries(
    Object.entries(configuredOptions).filter(([id]) => !represented.has(id))
  );

  /** 更新任意 config option，并保留其它已配置值。 */
  const updateOption = (id: string, value: RuntimeValue) => {
    onChange({ config_options: { ...configuredOptions, [id]: value } });
  };

  /** 返回标准类别控件，未握手时使用文本输入。 */
  const standardField = (category: string, configKey: string, label: string) => {
    const option = options.find((item) => item.category === category);
    if (!option) {
      return (
        <label className="settings-field" key={category}>
          <span>{label}</span>
          <input type="text" value={typeof acp[configKey] === "string" ? acp[configKey] as string : ""} onChange={(event) => onChange({ [configKey]: event.target.value })} />
        </label>
      );
    }
    return (
      <AcpOptionField
        key={option.id}
        option={option}
        value={(acp[configKey] as RuntimeValue | undefined) ?? option.currentValue}
        onChange={(value) => onChange({ [configKey]: value })}
      />
    );
  };

  return (
    <>
      {standardField("model", "model", t("ACP model", "ACP 模型"))}
      {standardField("mode", "permission_mode", t("ACP permission mode", "ACP 权限模式"))}
      {standardField("thought_level", "thought_level", t("ACP thought level", "ACP 思考等级"))}
      <div className="settings-field">
        <span>{t("ACP authentication method", "ACP 认证方式")}</span>
        {authMethods.length > 0 ? (
          <Select
            value={typeof acp.auth_method === "string" ? acp.auth_method : ""}
            options={[{ value: "", label: t("Not configured", "未配置") }, ...authMethods]}
            onChange={(value) => onChange({ auth_method: value })}
            ariaLabel={t("ACP authentication method", "ACP 认证方式")}
          />
        ) : (
          <input type="text" value={typeof acp.auth_method === "string" ? acp.auth_method : ""} onChange={(event) => onChange({ auth_method: event.target.value })} />
        )}
      </div>
      {options.filter((option) => !["model", "mode", "thought_level"].includes(option.category ?? "")).map((option) => (
        <AcpOptionField
          key={option.id}
          option={option}
          value={(configuredOptions[option.id] as RuntimeValue | undefined) ?? option.currentValue}
          onChange={(value) => updateOption(option.id, value)}
        />
      ))}
      {Object.keys(unmatched).length > 0 && (
        <div className="settings-field full">
          <span>{t("Other ACP config options", "其他 ACP 配置项")}</span>
          <StructuredConfigFields
            value={unmatched}
            onChange={(values) => onChange({ config_options: { ...configuredOptions, ...values } })}
          />
        </div>
      )}
    </>
  );
}

/**
 * 解析 initialize 响应中的认证方式。
 *
 * @param input agent 公布的认证方式
 * @returns 统一下拉框选项
 */
function parseAuthMethods(input: unknown): Array<{ value: string; label: string; description?: string }> {
  if (!Array.isArray(input)) return [];
  return input.flatMap((candidate) => {
    if (!candidate || typeof candidate !== "object") return [];
    const method = candidate as Record<string, unknown>;
    if (typeof method.id !== "string" || typeof method.name !== "string") return [];
    return [{
      value: method.id,
      label: method.name,
      ...(typeof method.description === "string" ? { description: method.description } : {})
    }];
  });
}

/**
 * 渲染单个动态 ACP 配置项。
 *
 * @param props 配置定义、当前值与更新回调
 * @returns 选择框或布尔开关
 */
function AcpOptionField({ option, value, onChange }: { option: AcpRuntimeOption; value: RuntimeValue; onChange: (value: RuntimeValue) => void }) {
  if (option.type === "boolean") {
    return (
      <label className="settings-toggle-field">
        <span><strong>{option.name}</strong><small>{option.description || option.id}</small></span>
        <input type="checkbox" checked={value === true} onChange={(event) => onChange(event.target.checked)} />
      </label>
    );
  }
  const selected = typeof value === "string" ? value : String(option.currentValue);
  return (
    <div className="settings-field">
      <span>{option.name}</span>
      <Select value={selected} options={option.values} onChange={onChange} ariaLabel={option.name} />
      {option.description && <small>{option.description}</small>}
    </div>
  );
}

/**
 * 展开 ACP 扁平或分组选择项。
 *
 * @param input 标准 select options
 * @returns 统一下拉框选项
 */
function flattenSelectOptions(input: unknown): AcpRuntimeOption["values"] {
  if (!Array.isArray(input)) return [];
  return input.flatMap((candidate) => {
    const nested = candidate && typeof candidate === "object"
      ? (candidate as Record<string, unknown>).options
      : undefined;
    const options = Array.isArray(nested) ? nested : [candidate];
    return options.flatMap((item) => {
      if (!item || typeof item !== "object") return [];
      const value = item as Record<string, unknown>;
      if (typeof value.value !== "string") return [];
      return [{
        value: value.value,
        label: typeof value.name === "string" ? value.name : value.value,
        ...(typeof value.description === "string" ? { description: value.description } : {})
      }];
    });
  });
}
