import { parseJsonRecord, stringField } from "./tool-data";
import { text, type Locale } from "../../i18n/locale";

const SUMMARY_FIELDS = ["query", "pattern", "command", "path", "package", "package_name", "tool_name", "group_name", "skill_name", "url", "task", "prompt", "description"];

/**
 * 提取适合在折叠工具卡头部展示的参数摘要。
 *
 * @param name 工具名称
 * @param argumentsText 工具参数 JSON 或参数预览
 * @returns 命令、加载目标、搜索词等紧凑摘要
 */
export function toolCardSummary(name: string, argumentsText: string, locale: Locale = "zh-CN"): string {
  const args = parseJsonRecord(argumentsText);
  if (!args) return compactText(argumentsText);
  if (name === "run_command" || name.includes("background_command")) {
    return stringField(args, "command") || stringField(args, "cmd");
  }
  if (name === "load") {
    return stringListField(args, "tool_names")
      || stringField(args, "tool_name")
      || stringField(args, "group_name")
      || stringField(args, "skill_name");
  }
  if (name === "review_aur_package" || name === "install_aur_package") {
    return stringField(args, "package");
  }
  if (name === "read_file") {
    const path = stringField(args, "path");
    if (path) return path;
    const files = Array.isArray(args.files) ? args.files : [];
    const firstPath = files.length > 0 && isRecord(files[0]) ? stringField(files[0], "path") : "";
    return firstPath
      ? `${firstPath}${files.length > 1 ? text(locale, ` and ${files.length - 1} more`, ` 等 ${files.length} 项`) : ""}`
      : text(locale, "Batch read", "批量读取");
  }
  for (const field of SUMMARY_FIELDS) {
    const value = stringField(args, field);
    if (value) return compactText(value);
  }
  return "";
}

/**
 * 将多行参数压缩为单行预览。
 *
 * @param value 原始参数文本
 * @returns 去除多余空白后的单行文本
 */
function compactText(value: string): string {
  return value.replace(/\s+/g, " ").trim();
}

/**
 * 将字符串数组字段转换为紧凑摘要。
 *
 * @param record 参数对象
 * @param field 字段名
 * @returns 去除空值后的逗号分隔摘要
 */
function stringListField(record: Record<string, unknown>, field: string): string {
  const value = record[field];
  if (!Array.isArray(value)) return "";
  return value
    .filter((item): item is string => typeof item === "string" && item.trim().length > 0)
    .map((item) => item.trim())
    .join(", ");
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
