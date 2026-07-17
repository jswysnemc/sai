import type { SessionTimelineTurn } from "../../api/contracts";
import type { LiveRunState, ToolLifecycle } from "./run-event-reducer";

export type MessageOverviewCategory = "history" | "live";

export type MessageOverviewStatus = "running" | "completed" | "interrupted" | "failed";

export type MessageOverviewItem = {
  id: string;
  category: MessageOverviewCategory;
  label: string;
  title: string;
  summary: string;
  tags: string[];
  hiddenTagCount: number;
  status: MessageOverviewStatus;
};

type OverviewTags = Pick<MessageOverviewItem, "tags" | "hiddenTagCount">;

const DEFAULT_TITLE_LENGTH = 56;
const DEFAULT_SUMMARY_LENGTH = 120;
const MAX_TAGS = 4;
const FILE_NAME_PATTERN = /\b[\w.-]+\.(?:c|cc|cpp|css|fish|go|h|hpp|html|java|js|json|jsonc|jsx|kt|md|py|rs|scss|sh|svelte|swift|toml|ts|tsx|vue|yaml|yml|zsh)\b/gi;
const FILE_PATH_PATTERN = /(?:^|[\s"'`(])((?:[\w.@+-]+[\\/])+[\w.@+-]+\.[a-z0-9]+)/gi;
const FILE_ARGUMENT_KEYS = new Set(["file", "files", "file_path", "file_paths", "filename", "path", "paths", "source", "target"]);

/**
 * 将会话时间线和可选实时状态转换为概览项。
 *
 * @param turns 按显示顺序排列的会话轮次
 * @param liveState 当前实时运行状态
 * @returns 可用于概览导航的有序项目
 */
export function createTimelineOverviewItems(
  turns: readonly SessionTimelineTurn[],
  liveState?: LiveRunState
): MessageOverviewItem[] {
  const items = turns.map(createHistoryOverviewItem);
  const liveItem = liveState ? createLiveOverviewItem(liveState) : null;
  return liveItem ? [...items, liveItem] : items;
}

/**
 * 将单个历史轮次转换为概览项。
 *
 * @param turn 会话时间线轮次
 * @returns 历史概览项
 */
export function createHistoryOverviewItem(turn: SessionTimelineTurn): MessageOverviewItem {
  const label = historyStatusLabel(turn.status);
  const overviewTags = collectOverviewTags(
    [turn.user.content, turn.assistant.content, turn.assistant.reasoning ?? ""],
    turn.tools.map((tool) => ({ arguments: tool.arguments, output: tool.output }))
  );
  return {
    id: `turn-${turn.turn_id}`,
    category: "history",
    label,
    title: createOverviewSummary(turn.user.content, DEFAULT_TITLE_LENGTH) || "未命名请求",
    summary: createOverviewSummary(turn.assistant.content, DEFAULT_SUMMARY_LENGTH) || label,
    ...overviewTags,
    status: turn.status
  };
}

/**
 * 将实时运行状态转换为概览项。
 *
 * @param state 当前实时运行状态
 * @returns 存在运行 ID 时返回实时概览项，否则返回 null
 */
export function createLiveOverviewItem(state: LiveRunState): MessageOverviewItem | null {
  if (!state.runId) return null;
  const label = liveStatusLabel(state);
  const overviewTags = collectOverviewTags(
    [state.userInput, state.content, state.reasoning],
    state.tools.map((tool) => ({ arguments: tool.arguments, output: tool.output }))
  );
  return {
    id: `live-${state.runId}`,
    category: "live",
    label,
    title: createOverviewSummary(state.userInput, DEFAULT_TITLE_LENGTH) || "未命名请求",
    summary: createOverviewSummary(state.content, DEFAULT_SUMMARY_LENGTH) || label,
    ...overviewTags,
    status: liveOverviewStatus(state)
  };
}

/**
 * 将 Markdown 内容转换为适合概览卡片展示的单行摘要。
 *
 * @param content 原始 Markdown 内容
 * @param maxLength 摘要允许的最大字符数，包含省略号
 * @returns 清理并截断后的单行文本
 */
export function createOverviewSummary(content: string | null | undefined, maxLength = DEFAULT_SUMMARY_LENGTH): string {
  if (!content || maxLength <= 0) return "";
  const plainText = stripMarkdown(content).replace(/\s+/g, " ").trim();
  if (Array.from(plainText).length <= maxLength) return plainText;
  if (maxLength === 1) return "…";
  return `${Array.from(plainText).slice(0, maxLength - 1).join("")}…`;
}

/**
 * 按固定优选间距计算居中的概览标识位置。
 *
 * @param index 当前项目序号
 * @param itemCount 项目总数
 * @param trackHeight 可用轨道高度
 * @param preferredGap 标识之间的优选间距
 * @returns 当前标识相对轨道顶部的位置
 */
export function evenlySpacedOverviewPosition(
  index: number,
  itemCount: number,
  trackHeight: number,
  preferredGap = 14
): number {
  if (itemCount <= 1) return Math.max(trackHeight, 0) / 2;
  const safeTrackHeight = Math.max(trackHeight, 0);
  const safeIndex = Math.min(Math.max(index, 0), itemCount - 1);
  const gap = Math.min(Math.max(preferredGap, 0), safeTrackHeight / (itemCount - 1));
  const groupHeight = gap * (itemCount - 1);
  return (safeTrackHeight - groupHeight) / 2 + safeIndex * gap;
}

/**
 * 清理 Markdown 结构符号并保留可读文本。
 *
 * @param content 原始 Markdown 内容
 * @returns 移除 Markdown 表记后的文本
 */
function stripMarkdown(content: string): string {
  return content
    .replace(/```[^\n]*\n?([\s\S]*?)```/g, "$1")
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/<[^>]+>/g, " ")
    .replace(/^\s{0,3}(?:#{1,6}\s+|>\s?|[-+*]\s+|\d+[.)]\s+)/gm, "")
    .replace(/^\s*\|?\s*:?-{3,}:?\s*(?:\|\s*:?-{3,}:?\s*)+\|?\s*$/gm, " ")
    .replace(/[|]/g, " ")
    .replace(/[*~`]/g, "");
}

/**
 * 从消息正文和工具字段中收集去重后的文件标签。
 *
 * @param contents 可包含文件路径的消息正文
 * @param tools 工具参数和输出
 * @returns 前四个文件名标签及其余标签数量
 */
function collectOverviewTags(contents: readonly string[], tools: readonly Pick<ToolLifecycle, "arguments" | "output">[]): OverviewTags {
  const tags = new Set<string>();
  for (const content of contents) collectFileNames(content, tags);
  for (const tool of tools) {
    collectArgumentFileNames(tool.arguments, tags);
    collectFileNames(tool.output, tags);
  }
  const allTags = Array.from(tags);
  return {
    tags: allTags.slice(0, MAX_TAGS),
    hiddenTagCount: Math.max(0, allTags.length - MAX_TAGS)
  };
}

/**
 * 从工具 JSON 参数中的文件字段提取文件名。
 *
 * @param argumentsText 工具参数文本
 * @param tags 文件标签集合
 * @returns 无返回值
 */
function collectArgumentFileNames(argumentsText: string, tags: Set<string>): void {
  if (!argumentsText.trim()) return;
  try {
    const value: unknown = JSON.parse(argumentsText);
    visitArgumentValue(value, "", tags);
  } catch {
    collectFileNames(argumentsText, tags);
  }
}

/**
 * 递归访问工具参数，仅采集文件语义字段。
 *
 * @param value 当前参数值
 * @param key 当前字段名
 * @param tags 文件标签集合
 * @returns 无返回值
 */
function visitArgumentValue(value: unknown, key: string, tags: Set<string>): void {
  if (typeof value === "string") {
    if (FILE_ARGUMENT_KEYS.has(key.toLowerCase())) collectFileNames(value, tags);
    return;
  }
  if (Array.isArray(value)) {
    for (const entry of value) visitArgumentValue(entry, key, tags);
    return;
  }
  if (!value || typeof value !== "object") return;
  for (const [childKey, childValue] of Object.entries(value)) visitArgumentValue(childValue, childKey, tags);
}

/**
 * 从普通文本中提取路径或文件名。
 *
 * @param text 待扫描文本
 * @param tags 文件标签集合
 * @returns 无返回值
 */
function collectFileNames(text: string, tags: Set<string>): void {
  if (!text) return;
  for (const match of text.matchAll(FILE_PATH_PATTERN)) {
    tags.add(baseName(match[1]));
  }
  for (const match of text.matchAll(FILE_NAME_PATTERN)) {
    tags.add(baseName(match[0]));
  }
}

/**
 * 从跨平台路径中获取末尾文件名。
 *
 * @param path 文件路径
 * @returns 文件名
 */
function baseName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}

/**
 * 将历史轮次状态转换为显示标签。
 *
 * @param status 历史轮次状态
 * @returns 中文状态标签
 */
function historyStatusLabel(status: SessionTimelineTurn["status"]): string {
  if (status === "running") return "正在处理";
  if (status === "interrupted") return "已中断";
  return "已完成";
}

/**
 * 将实时状态转换为显示标签。
 *
 * @param state 当前实时运行状态
 * @returns 中文状态标签
 */
function liveStatusLabel(state: LiveRunState): string {
  if (state.status === "queued") return "排队中";
  if (state.error) return "运行失败";
  if (state.completed) return "已完成";
  if (state.status === "waiting_response") return "等待响应";
  if (state.status === "thinking") return "思考中";
  if (state.status === "working") return "工作中";
  return "等待开始";
}

/**
 * 将实时状态转换为概览稳定状态。
 *
 * @param state 当前实时运行状态
 * @returns 概览状态
 */
function liveOverviewStatus(state: LiveRunState): MessageOverviewStatus {
  if (state.error) return "failed";
  if (state.completed) return "completed";
  return "running";
}
