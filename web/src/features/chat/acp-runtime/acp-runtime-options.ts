import type { EngineStatusResponse } from "../../../api/contracts";

/** 单个 ACP 配置项的取值类型 */
export type AcpOptionValue = string | boolean;

/** agent 上报的可渲染配置项 */
export type AcpRuntimeOption = {
  id: string;
  name: string;
  description: string;
  /** 标准类别；model / mode / thought_level 由专用控件承担 */
  category?: string;
  type: "select" | "boolean";
  currentValue: AcpOptionValue;
  values: Array<{ value: string; label: string; description?: string }>;
};

/** 由主页面专用控件承担的标准类别 */
const DEDICATED_CATEGORIES = ["model", "thought_level"];

/**
 * 将 ACP 运行状态解析成可渲染配置项。
 *
 * @param input agent 公布的标准 configOptions
 * @returns 已过滤的选择项与布尔项
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
        type: "boolean",
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
      type: "select",
      currentValue: option.currentValue,
      values
    }];
  });
}

/**
 * 返回需要在主页面弹层里展示的配置项。
 *
 * 模型与思考等级已有专用选择器，这里排除它们以免重复；权限模式虽然也是
 * 标准类别，但主页面没有对应控件，仍由弹层承担。
 *
 * @param status 当前内核运行状态
 * @returns 待展示的配置项
 */
export function acpAdjustableOptions(
  status: EngineStatusResponse | undefined
): AcpRuntimeOption[] {
  return parseAcpRuntimeOptions(status?.acp_runtime?.config_options).filter(
    (option) => !DEDICATED_CATEGORIES.includes(option.category ?? "")
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
