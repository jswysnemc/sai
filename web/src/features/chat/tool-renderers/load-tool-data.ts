import { parseJsonRecord, type JsonRecord } from "./tool-data";

/** `load` 读取到的一份 Skill 文档。 */
export type LoadedSkill = {
  name: string;
  /** 文档 frontmatter 中的说明 */
  description: string;
  /** loaded 或 already_loaded */
  status: string;
  /** 去掉 frontmatter 的 Markdown 正文；already_loaded 时为空 */
  body: string;
};

/** `load` 读取到的一个工具 Schema。 */
export type LoadedTool = {
  name: string;
  description: string;
  status: string;
  /** 参数名、类型与是否必填 */
  parameters: { name: string; type: string; required: boolean; description: string }[];
};

/** `load` 结果：Skill 文档或工具 Schema。 */
export type LoadResult =
  | { kind: "skill"; items: LoadedSkill[] }
  | { kind: "tool"; items: LoadedTool[] };

/**
 * 解析 `load` 工具结果。
 *
 * @param output 工具输出 JSON
 * @returns Skill 或工具结果；不是成功的 load 结果时返回 null
 */
export function parseLoadResult(output: string): LoadResult | null {
  const record = parseJsonRecord(output);
  if (!record || record.ok !== true) return null;
  if (Array.isArray(record.skills)) {
    return { kind: "skill", items: record.skills.flatMap(skillOf) };
  }
  if (Array.isArray(record.tools)) {
    return { kind: "tool", items: record.tools.flatMap(toolOf) };
  }
  return null;
}

/**
 * 读取一条 Skill 结果并拆出 frontmatter。
 *
 * @param value 结果条目
 * @returns Skill；名称为空时丢弃
 */
function skillOf(value: unknown): LoadedSkill[] {
  if (!isRecord(value)) return [];
  const name = stringOf(value.name);
  if (!name) return [];
  const { fields, body } = splitFrontmatter(stringOf(value.content));
  return [{
    name,
    description: fields.description ?? "",
    status: stringOf(value.status) || "loaded",
    body
  }];
}

/**
 * 读取一条工具 Schema 结果。
 *
 * @param value 结果条目
 * @returns 工具；名称为空时丢弃
 */
function toolOf(value: unknown): LoadedTool[] {
  if (!isRecord(value)) return [];
  const name = stringOf(value.name);
  if (!name) return [];
  const definition = isRecord(value.definition) ? value.definition : {};
  const fn = isRecord(definition.function) ? definition.function : {};
  const schema = isRecord(fn.parameters) ? fn.parameters : {};
  const properties = isRecord(schema.properties) ? schema.properties : {};
  const required = new Set(Array.isArray(schema.required) ? schema.required.filter((item): item is string => typeof item === "string") : []);
  const parameters = Object.entries(properties).map(([key, raw]) => {
    const spec = isRecord(raw) ? raw : {};
    return {
      name: key,
      type: typeName(spec),
      required: required.has(key),
      description: stringOf(spec.description)
    };
  });
  return [{ name, description: stringOf(fn.description), status: stringOf(value.status) || "loaded", parameters }];
}

/**
 * 拆出 YAML frontmatter 中的简单键值与正文。
 *
 * @param content SKILL.md 全文
 * @returns frontmatter 字段与正文
 */
export function splitFrontmatter(content: string): { fields: Record<string, string>; body: string } {
  const match = /^---\r?\n([\s\S]*?)\r?\n---\r?\n?/u.exec(content);
  if (!match) return { fields: {}, body: content.trim() };
  const fields: Record<string, string> = {};
  for (const line of (match[1] ?? "").split(/\r?\n/u)) {
    const pair = /^([A-Za-z_][\w-]*):\s*(.*)$/u.exec(line);
    if (pair) fields[pair[1] ?? ""] = (pair[2] ?? "").replace(/^["']|["']$/gu, "").trim();
  }
  return { fields, body: content.slice(match[0].length).trim() };
}

/**
 * 参数类型名：数组带上元素类型，枚举显示为 enum。
 *
 * @param spec 参数 Schema
 * @returns 类型名
 */
function typeName(spec: JsonRecord): string {
  if (Array.isArray(spec.enum)) return "enum";
  const type = stringOf(spec.type);
  if (type === "array" && isRecord(spec.items)) {
    const inner = stringOf(spec.items.type);
    return inner ? `${inner}[]` : "array";
  }
  return type || "any";
}

/**
 * 读取字符串字段。
 *
 * @param value 字段值
 * @returns 去掉首尾空白的字符串
 */
function stringOf(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
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
