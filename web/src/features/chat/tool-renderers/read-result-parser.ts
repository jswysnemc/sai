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

/** 图片读取的来源与尺寸说明。 */
export type ImageReadNote = {
  source: string;
  summary: string;
};

/**
 * 解析 read_file 单文件或批量文本结果。
 *
 * 历史记录是 text-page JSON；当前结果是 `行号<TAB>正文`。
 *
 * @param output read_file 输出
 * @returns 可着色渲染的文本分页列表
 */
export function parseReadTextPages(output: string): ReadTextPage[] {
  const trimmed = output.trim();
  if (!trimmed) return [];
  const result = parseJsonRecord(trimmed);
  if (result) {
    if (stringField(result, "type") === "text-page") {
      const page = parseTextPage(result);
      return page ? [page] : [];
    }
    if (stringField(result, "type") === "multi-text-page" && Array.isArray(result.results)) {
      return result.results.flatMap((item) => {
        if (!isRecord(item) || stringField(item, "type") !== "text-page") return [];
        const page = parseTextPage(item);
        return page ? [page] : [];
      });
    }
  }
  const numbered = parseNumberedText(trimmed);
  return numbered ? [numbered] : [];
}

/**
 * 解析图片读取说明。
 *
 * 图片本体在模型附件里，工具文本只保留来源、格式、大小和尺寸。
 *
 * @param output read_file 输出
 * @returns 图片说明；不是图片元信息时返回 null
 */
export function parseImageReadNote(output: string): ImageReadNote | null {
  const trimmed = output.trim();
  if (!trimmed.startsWith("[Image:") || !trimmed.endsWith("]")) return null;
  const summary = trimmed.slice("[Image:".length, -1).trim();
  if (!summary) return null;
  const source = /^source:\s*([^,]+)/u.exec(summary)?.[1]?.trim() ?? "";
  return { source, summary };
}

/**
 * 解析带有后端行号前缀的文本内容。
 *
 * @param content 形如 "12\tsource" 或历史 "12: source" 的多行文本
 * @returns 行号和源代码正文列表
 */
export function parseReadLines(content: string): ReadResultLine[] {
  return content.split("\n").map((line) => {
    const tab = /^(\d+)\t(.*)$/u.exec(line);
    if (tab) return { number: Number(tab[1]), text: tab[2] };
    const colon = /^(\d+): (.*)$/su.exec(line);
    return colon ? { number: Number(colon[1]), text: colon[2] } : { number: null, text: line };
  });
}

/**
 * 把整段 `行号<TAB>正文` 转成一个文本分页。
 *
 * @param output 工具输出
 * @returns 文本分页；夹杂非行号行时返回 null
 */
function parseNumberedText(output: string): ReadTextPage | null {
  const rawLines = output.split("\n").filter((line) => line.length > 0);
  if (rawLines.length === 0 || rawLines.some((line) => !/^(\d+)\t/u.test(line))) return null;
  const lines = parseReadLines(rawLines.join("\n"));
  const offset = lines[0]?.number ?? null;
  return {
    path: "",
    offset,
    limit: null,
    lineCount: lines.length,
    truncated: false,
    next: null,
    content: rawLines.join("\n"),
    lines
  };
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
