import type { LiveMessagePart, ToolLifecycle } from "../run-event-reducer";
import {
  isCommandToolName,
  isEditToolName,
  isExploreToolName,
  toolDisplaySummary
} from "../tool-renderers/tool-display-summary";
import { isReadOnlyShellCommand } from "../tool-renderers/read-only-command";
import { parseJsonRecord, stringField } from "../tool-renderers/tool-data";
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
 * 为工具组生成分类计数式操作说明。
 *
 * 形如「探索了 3 个文件，运行了 2 个命令」——按探索/命令/编辑/其他分桶计数，
 * 而不是罗列具体对象；对象细节交给展开态与折叠行轮播。
 * 只读 shell 命令（cat、ls、grep 等）计入探索桶。
 *
 * @param tools 工具组中的完成项
 * @param locale 界面语言
 * @param workspacePath 当前工作区路径，用于去重时归一路径
 * @returns 分类计数标题
 */
export function toolCallGroupLabel(
  tools: ToolLifecycle[],
  locale: Locale = "zh-CN",
  workspacePath = ""
): string {
  if (tools.every((tool) => tool.name === "todo")) {
    return text(locale, `Updated the plan ${tools.length} times`, `更新了 ${tools.length} 次计划`);
  }

  // 1. 按探索/命令/编辑/其他分桶；探索与编辑按对象去重，重复读写同一文件只算一个
  const exploreTools: ToolLifecycle[] = [];
  const editTools: ToolLifecycle[] = [];
  let commands = 0;
  let others = 0;
  for (const tool of tools) {
    if (isExploreTool(tool)) exploreTools.push(tool);
    else if (isCommandTool(tool)) commands += 1;
    else if (isEditTool(tool)) editTools.push(tool);
    else others += 1;
  }
  const explores = uniqueTargetCount(exploreTools, locale, workspacePath);
  const edits = uniqueTargetCount(editTools, locale, workspacePath);

  // 2. 逐桶拼接计数段，空桶不占位
  const segments: string[] = [];
  if (explores > 0) {
    segments.push(text(locale, `Explored ${explores} ${plural(explores, "file")}`, `探索了 ${explores} 个文件`));
  }
  if (commands > 0) {
    segments.push(text(locale, `Ran ${commands} ${plural(commands, "command")}`, `运行了 ${commands} 个命令`));
  }
  if (edits > 0) {
    segments.push(text(locale, `Edited ${edits} ${plural(edits, "file")}`, `编辑了 ${edits} 个文件`));
  }
  if (others > 0) {
    segments.push(text(locale, `Used ${others} ${plural(others, "tool")}`, `执行了 ${others} 个工具`));
  }
  if (segments.length === 0) {
    return text(locale, `Performed ${tools.length} operations`, `执行了 ${tools.length} 项操作`);
  }
  return segments.join(text(locale, ", ", "，"));
}

/**
 * 判断工具是否为阅读/搜索类探索操作。
 *
 * 只读 shell 命令与 read/grep 工具同属探索行为，一并归入。
 *
 * @param tool 工具生命周期
 * @returns 探索类返回 true
 */
export function isExploreTool(tool: ToolLifecycle): boolean {
  if (isExploreToolName(tool.name)) return true;
  return isCommandToolName(tool.name) && isReadOnlyShellCommand(commandTextOf(tool));
}

/**
 * 判断工具是否为写操作类 Shell / 后台命令。
 *
 * 只读命令归探索桶，这里保持互斥，避免同一项被计两次。
 *
 * @param tool 工具生命周期
 * @returns 命令类返回 true
 */
export function isCommandTool(tool: ToolLifecycle): boolean {
  return isCommandToolName(tool.name) && !isReadOnlyShellCommand(commandTextOf(tool));
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
 * 提取命令类工具的完整命令文本。
 *
 * @param tool 工具生命周期
 * @returns 参数中的 command/cmd 字段；缺失时返回空串
 */
function commandTextOf(tool: ToolLifecycle): string {
  const args = parseJsonRecord(tool.arguments || tool.argumentsPreview);
  if (!args) return "";
  return stringField(args, "command") || stringField(args, "cmd");
}

/**
 * 统计一组工具的去重操作对象数。
 *
 * 以展示摘要作为去重键——read 与 grep 指向同一路径时摘要一致，自然并为一个；
 * 提不出摘要的调用无法辨认对象，各算一个。
 *
 * @param tools 同一桶内的工具
 * @param locale 界面语言
 * @param workspacePath 当前工作区路径
 * @returns 去重后的对象数
 */
function uniqueTargetCount(tools: ToolLifecycle[], locale: Locale, workspacePath: string): number {
  const seen = new Set<string>();
  let anonymous = 0;
  for (const tool of tools) {
    const summary = toolDisplaySummary(
      tool.name,
      tool.arguments || tool.argumentsPreview || "",
      locale,
      workspacePath
    ).trim();
    if (summary) seen.add(summary.toLowerCase());
    else anonymous += 1;
  }
  return seen.size + anonymous;
}

/**
 * 返回英文名词的单复数形式。
 *
 * @param count 数量
 * @param noun 单数名词
 * @returns count 为 1 时原样返回，否则加 s
 */
function plural(count: number, noun: string): string {
  return count === 1 ? noun : `${noun}s`;
}
