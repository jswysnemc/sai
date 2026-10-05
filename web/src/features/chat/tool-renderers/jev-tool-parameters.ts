import type { JsonRecord } from "./tool-data";

/** 工具 Schema 中的一个参数。 */
export type JevToolParameter = {
  name: string;
  /** JSON Schema 类型；联合类型以 ` | ` 连接 */
  type: string;
  required: boolean;
  description: string;
};

/**
 * 【Jev】【工具参数】把工具定义里的 JSON Schema 收成参数列表。
 *
 * 只读取顶层 `properties`，嵌套对象只显示类型，不展开子字段。
 *
 * @param definition 工具定义（`{ function: { parameters } }` 或直接是 parameters）
 * @returns 参数列表；没有 Schema 时为空数组
 */
export function toolParametersOf(definition: unknown): JevToolParameter[] {
  const schema = parametersSchema(definition);
  if (!schema) return [];
  const properties = schema.properties;
  if (!isRecord(properties)) return [];
  const required = new Set(
    Array.isArray(schema.required) ? schema.required.filter((item): item is string => typeof item === "string") : []
  );
  return Object.entries(properties).map(([name, value]) => {
    const property = isRecord(value) ? value : {};
    return {
      name,
      type: schemaType(property),
      required: required.has(name),
      description: typeof property.description === "string" ? property.description.trim() : ""
    };
  });
}

/**
 * 定位工具定义里的参数 Schema。
 *
 * @param definition 工具定义
 * @returns 参数 Schema；找不到时为空
 */
function parametersSchema(definition: unknown): JsonRecord | null {
  if (!isRecord(definition)) return null;
  const functionDef = definition.function;
  if (isRecord(functionDef) && isRecord(functionDef.parameters)) return functionDef.parameters;
  if (isRecord(definition.parameters)) return definition.parameters;
  return null;
}

/**
 * 读取参数的类型说明。
 *
 * @param property 单个参数 Schema
 * @returns 类型文本，如 `string`、`string | null`、`enum`
 */
function schemaType(property: JsonRecord): string {
  if (typeof property.type === "string") return property.type;
  if (Array.isArray(property.type)) {
    return property.type.filter((item): item is string => typeof item === "string").join(" | ");
  }
  if (Array.isArray(property.enum)) return "enum";
  return "";
}

/**
 * 判断未知值是否为普通对象。
 *
 * @param value 待判断值
 * @returns 是否可按字段读取
 */
function isRecord(value: unknown): value is JsonRecord {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
