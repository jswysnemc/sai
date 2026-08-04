import { parseJsonRecord, stringField } from "./tool-data";

export type ReadResultLine = {
  number: number | null;
  text: string;
};

export type ReadTextPage = {
  path: string;
  offset: number | null;
  limit: number | null;
  /** 本次读取实际返回的行数 */
  lineCount: number;
  /** 是否因预算截断，仍有后续内容 */
  truncated: boolean;
  /** 续读起始行号，未截断时为 null */
  next: number | null;
  content: string;
  lines: ReadResultLine[];
};

/**
 * 解析 read_file 单文件或批量文本结果。
 *
 * @param output read_file 输出 JSON
 * @returns 可着色渲染的文本分页列表
 */
export function parseReadTextPages(output: string): ReadTextPage[] {
  const result = parseJsonRecord(output);
  if (!result) return [];
  if (stringField(result, "type") === "text-page") {
    const page = parseTextPage(result);
    return page ? [page] : [];
  }
  if (stringField(result, "type") !== "multi-text-page" || !Array.isArray(result.results)) return [];
  return result.results.flatMap((item) => {
    if (!isRecord(item) || stringField(item, "type") !== "text-page") return [];
    const page = parseTextPage(item);
    return page ? [page] : [];
  });
}

/**
 * 解析带有后端行号前缀的文本内容。
 *
 * @param content 形如 "12: source" 的多行文本
 * @returns 行号和源代码正文列表
 */
export function parseReadLines(content: string): ReadResultLine[] {
  return content.split("\n").map((line) => {
    const match = /^(\d+): (.*)$/s.exec(line);
    return match ? { number: Number(match[1]), text: match[2] } : { number: null, text: line };
  });
}

/** 把单个 JSON 对象转换为文本分页。 */
function parseTextPage(record: Record<string, unknown>): ReadTextPage | null {
  const path = stringField(record, "path");
  const rawContent = record.content;
  if (!path || typeof rawContent !== "string") return null;
  const content = rawContent;
  const lines = parseReadLines(content);
  return {
    path,
    offset: numberField(record, "offset"),
    limit: numberField(record, "limit"),
    // 空内容会被 split 成单个空串，按 0 行计
    lineCount: content.length === 0 ? 0 : lines.length,
    truncated: record.truncated === true,
    next: numberField(record, "next"),
    content,
    lines
  };
}

/** 读取有限数字字段。 */
function numberField(record: Record<string, unknown>, key: string): number | null {
  const value = record[key];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

/** 判断未知值是否为普通对象。 */
function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
