import { parseJsonRecord, stringField } from "../tool-renderers/tool-data";

export type TurnFileChange = {
  path: string;
  action: string;
  added: number;
  removed: number;
  tool: string;
};

type ToolLike = {
  name: string;
  arguments?: string;
  argumentsPreview?: string;
  output?: string;
  status?: string;
  ok?: boolean | null;
};

const EDIT_TOOLS = new Set(["edit_file", "write_file", "str_replace"]);

/**
 * 从本轮工具结果汇总文件改动。
 *
 * @param tools 工具生命周期列表
 * @returns 按路径合并后的改动列表
 */
export function collectTurnFileChanges(tools: readonly ToolLike[]): TurnFileChange[] {
  const merged = new Map<string, TurnFileChange>();
  for (const tool of tools) {
    if (!EDIT_TOOLS.has(tool.name)) continue;
    if (tool.status && tool.status !== "completed") continue;
    if (tool.ok === false) continue;
    const argsText = tool.arguments || tool.argumentsPreview || "";
    const fromOutput = changesFromOutput(tool.output || "", tool.name);
    const entries = fromOutput.length > 0 ? fromOutput : changesFromArguments(argsText, tool.name);
    for (const entry of entries) {
      const current = merged.get(entry.path);
      if (!current) {
        merged.set(entry.path, { ...entry });
        continue;
      }
      current.added += entry.added;
      current.removed += entry.removed;
      current.action = mergeAction(current.action, entry.action);
    }
  }
  return [...merged.values()].sort((left, right) => left.path.localeCompare(right.path));
}

/**
 * 从工具输出 JSON 提取 changed_files。
 *
 * @param output 工具输出
 * @param tool 工具名
 * @returns 改动列表
 */
function changesFromOutput(output: string, tool: string): TurnFileChange[] {
  const result = parseJsonRecord(output);
  const files = Array.isArray(result?.changed_files) ? result.changed_files : [];
  return files.flatMap((item) => {
    if (!item || typeof item !== "object" || Array.isArray(item)) return [];
    const record = item as Record<string, unknown>;
    const path = typeof record.path === "string" ? record.path : "";
    if (!path) return [];
    return [{
      path,
      action: typeof record.action === "string" ? record.action : "Edited",
      added: typeof record.added === "number" ? record.added : 0,
      removed: typeof record.removed === "number" ? record.removed : 0,
      tool
    }];
  });
}

/**
 * 在输出缺失时从参数兜底提取路径。
 *
 * @param argumentsText 参数 JSON
 * @param tool 工具名
 * @returns 改动列表
 */
function changesFromArguments(argumentsText: string, tool: string): TurnFileChange[] {
  const args = parseJsonRecord(argumentsText);
  if (!args) return [];
  if (tool === "write_file" || tool === "str_replace") {
    const path = stringField(args, "path");
    return path ? [{ path, action: tool === "write_file" ? "Edited" : "Edited", added: 0, removed: 0, tool }] : [];
  }
  if (tool === "edit_file") {
    return stringField(args, "patch")
      .split("\n")
      .flatMap((line) => {
        const match = /^\*\*\* (Add|Delete|Update) File: (.+)$/.exec(line);
        if (!match) return [];
        const action = match[1] === "Add" ? "Added" : match[1] === "Delete" ? "Deleted" : "Edited";
        return [{ path: match[2].trim(), action, added: 0, removed: 0, tool }];
      });
  }
  return [];
}


/**
 * 合并同一路径的动作标签。
 *
 * @param current 已有动作
 * @param next 新动作
 * @returns 合并后的动作
 */
function mergeAction(current: string, next: string): string {
  if (current === next) return current;
  if (current === "Added" && next === "Deleted") return "Deleted";
  if (current === "Added") return "Added";
  if (next === "Deleted") return "Deleted";
  if (next === "Renamed") return "Renamed";
  return current === "Renamed" ? current : "Edited";
}
