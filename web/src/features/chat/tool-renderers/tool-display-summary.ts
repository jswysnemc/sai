import { lenientStringField, parseJsonRecord, stringField } from "./tool-data";
import { text, type Locale } from "../../i18n/locale";
import { workspaceRelativePath } from "../../workspace/workspace-path-utils";

/** 单条摘要最大字符数 */
const MAX_ITEM_CHARS = 36;

/**
 * 提取适合工具卡头部与工具组标题展示的可读摘要。
 *
 * @param name 工具名称
 * @param argumentsText 参数 JSON 或预览
 * @param locale 界面语言
 * @param workspacePath 当前工作区绝对路径；有值时优先展示相对路径
 * @returns 可读摘要；无法提取时返回空串
 */
export function toolDisplaySummary(
  name: string,
  argumentsText: string,
  locale: Locale = "zh-CN",
  workspacePath = ""
): string {
  const args = parseJsonRecord(argumentsText);
  if (!args) {
    // 流式阶段常见未闭合 `{...`：不把碎片 JSON 塞进标题；能抠出字段再用
    if (looksLikeJsonFragment(argumentsText)) {
      const lenient = lenientSummaryFromPartial(name, argumentsText, locale, workspacePath);
      return lenient;
    }
    return compactText(argumentsText);
  }

  if (name === "run_command" || name.includes("background_command")) {
    // 后台任务管理操作（list/output/wait/stop/cleanup）没有 command 字段，
    // 摘要表达动作本身而不是退化成泛泛的「命令」
    if (name.includes("background_command")) {
      const action = stringField(args, "action");
      if (action && action !== "start") {
        const label = backgroundActionSummary(action, locale);
        const taskId = stringField(args, "task_id");
        return taskId ? compactText(`${label} ${taskId}`) : label;
      }
    }
    const command = stringField(args, "command") || stringField(args, "cmd");
    return humanizeCommand(command) || text(locale, "command", "命令");
  }

  if (name === "load") {
    const type = stringField(args, "type");
    const keywords = stringListField(args, "keywords");
    if (type && keywords) return compactText(`${type} · ${keywords}`);
    return (
      stringListField(args, "tool_names")
      || stringField(args, "tool_name")
      || stringField(args, "group_name")
      || stringField(args, "skill_name")
    );
  }

  if (name === "review_aur_package" || name === "install_aur_package") {
    return stringField(args, "package");
  }

  if (name === "read_file") {
    const path = stringField(args, "path");
    if (path) return displayPath(path, workspacePath);
    if (Array.isArray(args.files) && args.files.length > 0 && isRecord(args.files[0])) {
      const first = stringField(args.files[0], "path");
      if (first) {
        const label = displayPath(first, workspacePath);
        return args.files.length > 1
          ? text(locale, `${label} and more`, `${label} 等`)
          : label;
      }
    }
    return text(locale, "Batch read", "批量读取");
  }

  if (name === "write_file" || name === "str_replace") {
    const path = stringField(args, "path");
    if (path) return displayPath(path, workspacePath);
    return text(locale, "File edit", "文件修改");
  }

  if (name === "edit_file") {
    const path = firstPatchPath(stringField(args, "patch"));
    if (path) return displayPath(path, workspacePath);
    return text(locale, "File edit", "文件修改");
  }

  if (name === "trash_path") {
    const path = stringField(args, "path");
    return path ? displayPath(path, workspacePath) : text(locale, "Trash", "回收站");
  }

  if (name === "glob" || name === "find_files") {
    const path = stringField(args, "path");
    if (path) return displayPath(path, workspacePath);
    const pattern = stringField(args, "pattern");
    if (pattern && !isComplexPattern(pattern)) return compactText(pattern);
    return text(locale, "file matches", "文件匹配");
  }

  if (name === "grep" || name === "search_text") {
    const path = stringField(args, "path") || stringField(args, "include");
    if (path && !isComplexPattern(path)) return displayPath(path, workspacePath);
    const pattern = stringField(args, "pattern") || stringField(args, "query");
    const human = humanizeSearchPattern(pattern);
    if (human) return human;
    return text(locale, "code search", "代码搜索");
  }

  if (
    name === "web_search"
    || name === "search_knowledge_base"
    || name === "search_knowledge_base_by_name"
    || name === "online_man_search"
    || name === "archwiki_query"
  ) {
    const query =
      stringField(args, "query")
      || stringField(args, "pattern")
      || stringField(args, "keyword");
    const human = humanizeSearchPattern(query);
    if (human) return human;
    return text(locale, "search", "搜索");
  }

  if (name === "web_fetch" || name === "online_man_get_page") {
    const url = stringField(args, "url") || stringField(args, "page");
    if (url) return compactText(url.replace(/^https?:\/\//, ""));
    return text(locale, "page", "页面");
  }

  // 通用字段：路径优先，复杂 pattern 再做人话处理
  const path = stringField(args, "path");
  if (path && !isComplexPattern(path)) return displayPath(path, workspacePath);
  for (const field of ["query", "package", "package_name", "tool_name", "group_name", "skill_name", "url", "task", "description"] as const) {
    const value = stringField(args, field);
    if (value) return compactText(value);
  }
  const pattern = stringField(args, "pattern") || stringField(args, "command");
  if (pattern) {
    if (isComplexPattern(pattern)) {
      return humanizeSearchPattern(pattern) || humanizeCommand(pattern) || text(locale, "details", "详情");
    }
    return compactText(pattern);
  }
  return "";
}

/**
 * 将后台任务管理 action 转成可读摘要。
 *
 * @param action 后台任务操作（非 start）
 * @param locale 界面语言
 * @returns 动作标签
 */
function backgroundActionSummary(action: string, locale: Locale): string {
  const labels: Record<string, [string, string]> = {
    list: ["Task list", "任务列表"],
    output: ["Task output", "任务输出"],
    wait: ["Wait for task", "等待任务"],
    stop: ["Stop task", "停止任务"],
    cleanup: ["Clean up tasks", "清理任务"]
  };
  const label = labels[action];
  return label ? text(locale, label[0], label[1]) : action;
}

/**
 * 判断工具是否为阅读/搜索类探索操作。
 *
 * @param name 工具名称
 * @returns 探索类返回 true
 */
export function isExploreToolName(name: string): boolean {
  return (
    name === "read_file"
    || name === "glob"
    || name === "grep"
    || name === "web_search"
    || name === "web_fetch"
    || name === "search_text"
    || name === "find_files"
    || name === "search_knowledge_base"
    || name === "search_knowledge_base_by_name"
    || name === "read_knowledge_base_file"
    || name === "search_evicted_context"
    || name === "check_os_info"
    || name === "online_man_search"
    || name === "online_man_get_page"
    || name === "archwiki_query"
  );
}

/**
 * 判断工具是否为 Shell / 后台命令。
 *
 * @param name 工具名称
 * @returns 命令类返回 true
 */
export function isCommandToolName(name: string): boolean {
  return name === "run_command" || name.includes("command");
}

/**
 * 判断工具是否为文件编辑类。
 *
 * @param name 工具名称
 * @returns 编辑类返回 true
 */
export function isEditToolName(name: string): boolean {
  return name === "edit_file" || name === "write_file" || name === "str_replace";
}

/**
 * 将路径规范为工作区相对路径，并适度缩短。
 *
 * @param path 原始路径
 * @param workspacePath 工作区根路径
 * @returns 展示路径
 */
export function displayPath(path: string, workspacePath = ""): string {
  const relative = workspacePath
    ? workspaceRelativePath(path, workspacePath) || path
    : path;
  return shortPath(relative);
}

/**
 * 将复杂正则/杂乱 pattern 转成可读关键词。
 *
 * @param pattern 原始搜索模式
 * @returns 可读摘要；无法解析时返回空串
 */
export function humanizeSearchPattern(pattern: string): string {
  const raw = pattern.trim();
  if (!raw) return "";
  if (!isComplexPattern(raw) && raw.length <= MAX_ITEM_CHARS) return raw;
  // 1. 提取中文、英文标识符片段，过滤正则噪音
  const tokens = raw.match(/[A-Za-z\u4e00-\u9fff][\w\u4e00-\u9fff./-]{1,}/g) ?? [];
  const clean = tokens
    .map((token) => token.replace(/[./-]+$/g, ""))
    .filter((token) => token.length >= 2)
    .filter((token) => !isNoiseToken(token))
    .slice(0, 2);
  if (clean.length > 0) return clean.join(" ");
  return "";
}

/**
 * 将 shell 命令压成可展示的短标签。
 *
 * @param command 完整命令
 * @returns 首个可识别命令片段
 */
export function humanizeCommand(command: string): string {
  const single = command.replace(/\s+/g, " ").trim();
  if (!single) return "";
  const withoutEnv = single.replace(/^(?:[A-Za-z_][\w]*=\S+\s+)+/, "");
  const parts = withoutEnv.split(" ").filter(Boolean);
  if (parts.length === 0) return "";
  if (["npm", "pnpm", "yarn", "cargo", "git", "python", "python3", "node", "semble"].includes(parts[0])) {
    return compactText(parts.slice(0, 3).join(" "));
  }
  return compactText(parts.slice(0, 2).join(" "));
}

/**
 * 判断字符串是否像复杂正则或拼接关键词。
 *
 * @param value 候选文本
 * @returns 复杂模式返回 true
 */
export function isComplexPattern(value: string): boolean {
  if (value.length > MAX_ITEM_CHARS) return true;
  return /[|()[\]{}^$\\]/.test(value) || value.includes(".*") || value.includes(".+") || value.includes("?");
}

/**
 * 缩短路径，优先展示文件名与父目录。
 *
 * @param path 文件或目录路径
 * @returns 短路径
 */
export function shortPath(path: string): string {
  const normalized = path.replace(/\\/g, "/").replace(/\/+$/, "");
  if (normalized.length <= MAX_ITEM_CHARS) return normalized;
  const segments = normalized.split("/").filter(Boolean);
  if (segments.length === 0) return compactText(normalized);
  const file = segments[segments.length - 1];
  if (segments.length === 1) return compactText(file);
  const parent = segments[segments.length - 2];
  const short = `${parent}/${file}`;
  return compactText(short.length <= MAX_ITEM_CHARS ? short : file);
}

/**
 * 将多行参数压缩为单行预览。
 *
 * @param value 原始参数文本
 * @returns 去除多余空白后的单行文本
 */
function compactText(value: string): string {
  const single = value.replace(/\s+/g, " ").trim();
  if (single.length <= MAX_ITEM_CHARS) return single;
  return `${single.slice(0, MAX_ITEM_CHARS - 1)}…`;
}

/**
 * 判断文本是否像尚未闭合的 JSON 对象/数组碎片。
 *
 * @param value 参数预览
 * @returns 疑似 JSON 碎片时返回 true
 */
export function looksLikeJsonFragment(value: string): boolean {
  const trimmed = value.trimStart();
  return trimmed.startsWith("{") || trimmed.startsWith("[");
}

/**
 * 从未闭合 JSON 中宽松提取可读摘要（路径/命令等）。
 *
 * @param name 工具名
 * @param argumentsText 参数预览
 * @param locale 语言
 * @param workspacePath 工作区路径
 * @returns 可读摘要；无法提取时返回空串（隐藏碎片 JSON）
 */
function lenientSummaryFromPartial(
  name: string,
  argumentsText: string,
  locale: Locale,
  workspacePath: string
): string {
  if (name === "run_command" || name.includes("background_command")) {
    const command = lenientStringField(argumentsText, "command") || lenientStringField(argumentsText, "cmd");
    if (command) return humanizeCommand(command);
  }
  const path = lenientStringField(argumentsText, "path");
  if (path && (name === "read_file" || name === "write_file" || name === "str_replace" || name === "trash_path" || name === "glob" || name === "find_files")) {
    return displayPath(path, workspacePath);
  }
  const query =
    lenientStringField(argumentsText, "query")
    || lenientStringField(argumentsText, "pattern")
    || lenientStringField(argumentsText, "keyword");
  if (query && (name === "grep" || name === "search_text" || name.includes("search") || name.includes("web_"))) {
    return humanizeSearchPattern(query) || text(locale, "search", "搜索");
  }
  return "";
}

/**
 * 从 Codex patch 中提取首个文件路径。
 *
 * @param patch patch 文本
 * @returns 文件路径
 */
function firstPatchPath(patch: string): string {
  for (const line of patch.split("\n")) {
    if (
      line.startsWith("*** Add File: ")
      || line.startsWith("*** Delete File: ")
      || line.startsWith("*** Update File: ")
    ) {
      return line.slice(line.indexOf(": ") + 2).trim();
    }
  }
  return "";
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
 * 过滤正则/通用噪音词。
 *
 * @param token 候选词
 * @returns 噪音返回 true
 */
function isNoiseToken(token: string): boolean {
  const lower = token.toLowerCase();
  return [
    "tool",
    "fold",
    "command",
    "commands",
    "executed",
    "explored",
    "ran",
    "execute",
    "search",
    "pattern",
    "include",
    "tsx",
    "ts",
    "rs",
    "css",
    "执行了",
    "运行了",
    "探索了",
    "编辑了",
    "个命令",
    "项操作",
    "代码搜索",
    "文件匹配"
  ].includes(lower);
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
