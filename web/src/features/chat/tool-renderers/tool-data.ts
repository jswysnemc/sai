export type JsonRecord = Record<string, unknown>;

/**
 * 将 JSON 文本解析为对象。
 *
 * @param value JSON 文本
 * @returns 对象，解析失败时返回空值
 */
export function parseJsonRecord(value: string): JsonRecord | null {
  try {
    const parsed = JSON.parse(value) as unknown;
    return parsed !== null && typeof parsed === "object" && !Array.isArray(parsed) ? parsed as JsonRecord : null;
  } catch {
    return null;
  }
}

/**
 * 读取对象中的字符串字段。
 *
 * @param record JSON 对象
 * @param key 字段名
 * @returns 字符串字段或空字符串
 */
export function stringField(record: JsonRecord | null, key: string): string {
  const value = record?.[key];
  return typeof value === "string" ? value : "";
}

/**
 * 将任意 JSON 文本格式化为可读内容。
 *
 * @param value 原始文本
 * @returns 格式化文本
 */
export function prettyJson(value: string): string {
  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value;
  }
}

/**
 * 提取工具调用的紧凑摘要。
 *
 * @param name 工具名称
 * @param argumentsText 工具参数 JSON
 * @returns 路径、命令或搜索词摘要
 */
export function toolSummary(name: string, argumentsText: string): string {
  const args = parseJsonRecord(argumentsText);
  if (name === "run_command") return stringField(args, "command");
  if (name === "edit_file") {
    const path = stringField(args, "path");
    if (path) return path;
    const patch = stringField(args, "patch");
    return patch.split("\n").find((line) => line.startsWith("*** ") && line.includes(" File: "))?.split(" File: ")[1] ?? "文件修改";
  }
  if (name === "read_file") return stringField(args, "path") || "批量读取";
  if (name === "grep") return stringField(args, "pattern");
  if (name === "glob") return stringField(args, "pattern");
  return "";
}

/**
 * 提取适合在工具卡头部展示的唯一文件路径。
 *
 * @param name 工具名称
 * @param argumentsText 工具参数 JSON
 * @returns 唯一文件路径，多文件或无路径时返回空字符串
 */
export function toolFilePath(name: string, argumentsText: string): string {
  const args = parseJsonRecord(argumentsText);
  if (name === "read_file" || name === "edit_file") return stringField(args, "path");
  if (name !== "apply_patch") return "";
  const paths = stringField(args, "patch")
    .split("\n")
    .flatMap((line) => {
      const match = /^\*\*\* (?:Add|Delete|Update) File: (.+)$/.exec(line);
      return match ? [match[1].trim()] : [];
    });
  return new Set(paths).size === 1 ? paths[0] : "";
}
