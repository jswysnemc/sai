import type { LiveMessagePart, ToolLifecycle } from "../run-event-reducer";
import { text, type Locale } from "../../i18n/locale";

export type GroupedMessagePart =
  | { type: "part"; id: string; part: LiveMessagePart }
  | { type: "tool-group"; id: string; tools: ToolLifecycle[] };

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
 * 为工具组生成简短操作说明。
 *
 * @param tools 工具组中的完成项
 * @returns 命令组、计划组或通用操作组标题
 */
export function toolCallGroupLabel(tools: ToolLifecycle[], locale: Locale = "zh-CN"): string {
  if (tools.every((tool) => tool.name === "todo")) return text(locale, `Updated the plan ${tools.length} times`, `更新了 ${tools.length} 次计划`);
  const commandOnly = tools.every((tool) => tool.name === "run_command" || tool.name.includes("command"));
  return commandOnly
    ? text(locale, `Ran ${tools.length} commands`, `运行了 ${tools.length} 个命令`)
    : text(locale, `Performed ${tools.length} operations`, `执行了 ${tools.length} 项操作`);
}
