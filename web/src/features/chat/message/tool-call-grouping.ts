import type { LiveMessagePart, ToolLifecycle } from "../run-event-reducer";
import {
  isCommandToolName,
  isEditToolName,
  isExploreToolName,
  toolDisplaySummary
} from "../tool-renderers/tool-display-summary";
import { text, type Locale } from "../../i18n/locale";

export type GroupedMessagePart =
  | { type: "part"; id: string; part: LiveMessagePart }
  | { type: "tool-group"; id: string; tools: ToolLifecycle[] };

/** 组标题中最多展示的摘要条目数 */
const MAX_SUMMARY_ITEMS = 3;

/**
 * 单项摘要的最大字符数。
 *
 * 组标题要在一行内读完，而个别工具的摘要可能是一整句任务描述
 * （例如 subagent 的目标说明），不限长会把整行撑爆并挤掉其余条目。
 */
const MAX_SUMMARY_CHARS = 24;

/**
 * 聚合连续且已完成的工具调用，运行中和失败调用始终独立展示。
 *
 * @param parts 原始消息部件
 * @returns 保持原顺序的普通部件和工具组
 */
export function groupCompletedToolCalls(parts: LiveMessagePart[]): GroupedMessagePart[] {
  const result: GroupedMessagePart[] = [];
  let completedTools: Array<{ id: string; tool: ToolLifecycle }> = [];

  /** 将已收集的连续完成项写入结果。 */
  const flushCompleted = () => {
    if (completedTools.length >= 2) {
      result.push({
        type: "tool-group",
        // 仅用首项 id，组增长时不重挂载，避免展开状态被重置
        id: `tool-group-${completedTools[0].id}`,
        tools: completedTools.map((item) => item.tool)
      });
    } else if (completedTools.length === 1) {
      const item = completedTools[0];
      result.push({ type: "part", id: item.id, part: { id: item.id, type: "tool", tool: item.tool } });
    }
    completedTools = [];
  };

  for (const part of parts) {
    if (part.type === "tool" && part.tool.status === "completed") {
      completedTools.push({ id: part.id, tool: part.tool });
      continue;
    }
    flushCompleted();
    result.push({ type: "part", id: part.id, part });
  }
  flushCompleted();
  return result;
}

/**
 * 为工具组生成简短可读操作说明。
 *
 * @param tools 工具组中的完成项
 * @param locale 界面语言
 * @param workspacePath 当前工作区路径，用于相对路径展示
 * @returns 探索/执行/计划类摘要标题
 */
export function toolCallGroupLabel(
  tools: ToolLifecycle[],
  locale: Locale = "zh-CN",
  workspacePath = ""
): string {
  if (tools.every((tool) => tool.name === "todo")) {
    return text(locale, `Updated the plan ${tools.length} times`, `更新了 ${tools.length} 次计划`);
  }

  const items = tools
    .map((tool) =>
      clampSummary(
        toolDisplaySummary(
          tool.name,
          tool.arguments || tool.argumentsPreview || "",
          locale,
          workspacePath
        )
      )
    )
    .filter(Boolean);
  const unique = dedupePreserveOrder(items);
  const listed = formatLabelList(unique, MAX_SUMMARY_ITEMS, locale);
  if (!listed) {
    return text(locale, `Performed ${tools.length} operations`, `执行了 ${tools.length} 项操作`);
  }

  const exploreOnly = tools.every((tool) => isExploreToolName(tool.name));
  const commandOnly = tools.every((tool) => isCommandToolName(tool.name));
  const editOnly = tools.every((tool) => isEditToolName(tool.name));
  const hasExplore = tools.some((tool) => isExploreToolName(tool.name));
  const hasCommand = tools.some((tool) => isCommandToolName(tool.name));

  if (exploreOnly) {
    return text(locale, `Explored ${listed}`, `探索了 ${listed}`);
  }
  if (commandOnly) {
    return text(locale, `Ran ${listed}`, `执行了 ${listed}`);
  }
  if (editOnly) {
    return text(locale, `Edited ${listed}`, `编辑了 ${listed}`);
  }
  if (hasExplore && hasCommand) {
    return text(locale, `Explored and ran ${listed}`, `探索并执行了 ${listed}`);
  }
  if (hasExplore) {
    return text(locale, `Explored ${listed}`, `探索了 ${listed}`);
  }
  return text(locale, `Performed ${listed}`, `执行了 ${listed}`);
}

/**
 * 判断工具是否为阅读/搜索类探索操作。
 *
 * @param tool 工具生命周期
 * @returns 探索类返回 true
 */
export function isExploreTool(tool: ToolLifecycle): boolean {
  return isExploreToolName(tool.name);
}

/**
 * 判断工具是否为 Shell / 后台命令。
 *
 * @param tool 工具生命周期
 * @returns 命令类返回 true
 */
export function isCommandTool(tool: ToolLifecycle): boolean {
  return isCommandToolName(tool.name);
}

/**
 * 判断工具是否为文件编辑类。
 *
 * @param tool 工具生命周期
 * @returns 编辑类返回 true
 */
export function isEditTool(tool: ToolLifecycle): boolean {
  return isEditToolName(tool.name);
}

/**
 * 去重并保持首次出现顺序。
 *
 * @param items 摘要列表
 * @returns 去重后的列表
 */
function dedupePreserveOrder(items: string[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const item of items) {
    const key = item.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    result.push(item);
  }
  return result;
}

/**
 * 将摘要列表格式化为 “a、b、c 等”。
 *
 * @param items 去重后的摘要
 * @param maxItems 最多展示条数
 * @param locale 界面语言
 * @returns 展示文本
 */
function formatLabelList(items: string[], maxItems: number, locale: Locale): string {
  if (items.length === 0) return "";
  const visible = items.slice(0, maxItems);
  const joined = visible.join(locale.startsWith("zh") ? "、" : ", ");
  if (items.length > maxItems || items.length >= 3) {
    return text(locale, `${joined}, and more`, `${joined} 等`);
  }
  return joined;
}

/**
 * 截断过长的单项摘要。
 *
 * 优先在分隔符处断开，读起来像完整短语而不是被硬切一半。
 *
 * @param summary 原始摘要
 * @returns 不超过上限的摘要
 */
function clampSummary(summary: string): string {
  const trimmed = summary.trim();
  if (trimmed.length <= MAX_SUMMARY_CHARS) return trimmed;
  // 1. 优先在分隔符处断开，保留语义完整的前半段
  const head = trimmed.slice(0, MAX_SUMMARY_CHARS);
  const cut = Math.max(head.lastIndexOf(" - "), head.lastIndexOf("："), head.lastIndexOf(": "));
  if (cut > MAX_SUMMARY_CHARS / 2) return `${head.slice(0, cut).trim()}…`;
  // 2. 没有合适的断点时直接截断
  return `${head.trim()}…`;
}
