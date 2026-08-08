import { parseJsonRecord } from "./tool-data";

/** 单行展示的字段最大字符数，超过则改为整块展示 */
const INLINE_LIMIT = 72;

/** 参数字段的一条展示项 */
export type ToolField = {
  key: string;
  /** 已格式化的值文本 */
  value: string;
  /** 值是否需要独占多行 */
  block: boolean;
};

/**
 * 将工具参数 JSON 拆解为字段级展示项。
 *
 * 通用视图原先把整段 JSON 原样倾倒出来，读者要在花括号和引号里
 * 自己找出哪个键对应哪个值。拆成字段后短值排成两列、长值独占整块，
 * 参数结构一眼可见。
 *
 * @param source 参数 JSON 文本
 * @returns 字段列表；不是 JSON 对象时返回空数组
 */
export function parseToolFields(source: string): ToolField[] {
  const record = parseJsonRecord(source);
  if (!record) return [];
  const fields = Object.entries(record).map(([key, value]) => {
    const text = formatFieldValue(value);
    return { key, value: text, block: text.length > INLINE_LIMIT || text.includes("\n") };
  });
  // 短字段排在前：先给出参数轮廓，长文本正文往后放
  return fields.sort((left, right) => Number(left.block) - Number(right.block));
}

/**
 * 将字段值格式化为可展示文本。
 *
 * @param value 原始字段值
 * @returns 展示文本
 */
function formatFieldValue(value: unknown): string {
  if (value === null) return "null";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  // 数组与对象保留缩进：结构本身就是它们要表达的信息
  return JSON.stringify(value, null, 2);
}
