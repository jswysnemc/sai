import { parseReadTextPages } from "./read-result-parser";
import { parseJsonRecord } from "./tool-data";
import { text, type Locale } from "../../i18n/locale";

/**
 * 折叠行右侧的结果摘要。
 *
 * tone 决定这段文字的着色：成功的量级信息用中性色，
 * 失败的退出码用危险色，避免整行只靠一个红点表达失败。
 */
export type ToolResultSummary = {
  label: string;
  tone: "neutral" | "danger";
};

/** 增删统计单列成结构，供折叠行分别着色 */
export type ToolDiffStat = {
  added: number;
  removed: number;
};

/**
 * 从工具输出中提取折叠行可展示的结果摘要。
 *
 * 折叠态原本只说明"做了什么"，读完仍不知道"结果如何"，
 * 必须展开才能确认读到多少行、匹配到几处、命令是否成功。
 * 这里把量级结论提到折叠行，展开只用于查看细节。
 *
 * @param name 工具名称
 * @param output 工具输出文本
 * @param locale 界面语言
 * @returns 结果摘要；无可展示结论时返回 null
 */
export function toolResultSummary(
  name: string,
  output: string,
  locale: Locale = "zh-CN"
): ToolResultSummary | null {
  if (!output) return null;

  if (name === "read_file") return readSummary(output, locale);
  if (name === "grep" || name === "search_text") return matchSummary(output, locale);
  if (name === "glob" || name === "find_files" || name === "list_dir") return fileSummary(output, locale);
  if (name === "run_command" || name.includes("command")) return commandSummary(output, locale);
  return null;
}

/**
 * 从编辑类工具输出中提取增删行数。
 *
 * 与文字摘要分开返回：增删需要各自着色，压成一个字符串就没法分别染色。
 *
 * @param name 工具名称
 * @param output 工具输出文本
 * @returns 增删统计；非编辑类工具或无统计时返回 null
 */
export function toolDiffStat(name: string, output: string): ToolDiffStat | null {
  if (name !== "edit_file" && name !== "write_file" && name !== "str_replace") return null;
  const result = parseJsonRecord(output);
  if (!result || !Array.isArray(result.changed_files)) return null;
  let added = 0;
  let removed = 0;
  for (const file of result.changed_files) {
    if (!isRecord(file)) continue;
    added += numberOf(file.added);
    removed += numberOf(file.removed);
  }
  return added === 0 && removed === 0 ? null : { added, removed };
}

/**
 * 汇总 read_file 读取的总行数与文件数。
 *
 * @param output 读取结果 JSON
 * @param locale 界面语言
 * @returns 行数摘要
 */
function readSummary(output: string, locale: Locale): ToolResultSummary | null {
  const pages = parseReadTextPages(output);
  if (pages.length === 0) return null;
  const lines = pages.reduce((total, page) => total + page.lineCount, 0);
  if (lines === 0) return null;
  const lineLabel = text(locale, `${lines} lines`, `${lines} 行`);
  // 批量读取补上文件数，否则无法解释行数为何这么多
  const label = pages.length > 1
    ? text(locale, `${pages.length} files · ${lineLabel}`, `${pages.length} 个文件 · ${lineLabel}`)
    : lineLabel;
  return { label, tone: "neutral" };
}

/**
 * 统计搜索命中的行数。
 *
 * ripgrep 每行一条命中，因此行数即匹配数；后端在零命中时显式给出 matches 字段。
 *
 * @param output 搜索结果 JSON
 * @param locale 界面语言
 * @returns 匹配数摘要
 */
function matchSummary(output: string, locale: Locale): ToolResultSummary | null {
  const result = parseJsonRecord(output);
  if (!result) return null;
  if (result.matches === 0) {
    return { label: text(locale, "no matches", "无匹配"), tone: "neutral" };
  }
  const count = countLines(result.stdout);
  if (count === 0) return null;
  const suffix = result.truncated === true ? "+" : "";
  return {
    label: text(locale, `${count}${suffix} matches`, `${count}${suffix} 处匹配`),
    tone: "neutral"
  };
}

/**
 * 统计文件查找命中的条目数。
 *
 * @param output 查找结果 JSON
 * @param locale 界面语言
 * @returns 文件数摘要
 */
function fileSummary(output: string, locale: Locale): ToolResultSummary | null {
  const result = parseJsonRecord(output);
  if (!result) return null;
  const count = countLines(result.stdout);
  if (count === 0) return { label: text(locale, "no results", "无结果"), tone: "neutral" };
  const suffix = result.truncated === true ? "+" : "";
  return {
    label: text(locale, `${count}${suffix} entries`, `${count}${suffix} 项`),
    tone: "neutral"
  };
}

/**
 * 提取命令的失败退出码或后台提升说明。
 *
 * 前台超时会被提升为后台任务继续执行，这不是失败——结果里没有
 * exit_code，按失败渲染会把一次正常的提升标成"退出码未知"。
 * 成功的前台命令不占用摘要位：退出码 0 是默认预期，写出来只会挤占空间。
 *
 * @param output 命令结果 JSON
 * @param locale 界面语言
 * @returns 摘要；前台成功时返回 null
 */
function commandSummary(output: string, locale: Locale): ToolResultSummary | null {
  const result = parseJsonRecord(output);
  if (!result) return null;
  if (result.mode === "background") {
    return { label: text(locale, "moved to background", "已转入后台"), tone: "neutral" };
  }
  const exitCode = typeof result.exit_code === "number" ? result.exit_code : null;
  const success = typeof result.success === "boolean" ? result.success : exitCode === 0;
  if (success) return null;
  const label = exitCode === null
    ? text(locale, "failed", "执行失败")
    : text(locale, `exit ${exitCode}`, `退出码 ${exitCode}`);
  return { label, tone: "danger" };
}

/**
 * 统计文本的非空行数。
 *
 * @param value 待统计值
 * @returns 非空行数；非字符串时返回 0
 */
function countLines(value: unknown): number {
  if (typeof value !== "string") return 0;
  const trimmed = value.trim();
  if (!trimmed) return 0;
  return trimmed.split("\n").length;
}

/**
 * 读取有限数字，非数字按 0 计。
 *
 * @param value 待读取值
 * @returns 数字值
 */
function numberOf(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

/**
 * 判断未知值是否为普通对象。
 *
 * @param value 待判断值
 * @returns 是否可按 JSON 对象读取
 */
function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
