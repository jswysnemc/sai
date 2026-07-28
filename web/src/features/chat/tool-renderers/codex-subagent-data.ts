import { parseJsonRecord, stringField, type JsonRecord } from "./tool-data";

export type CodexSubagentActivityKind = "started" | "interacted" | "interrupted";

export type CodexSubagentActivity = {
  threadId: string;
  path: string;
  name: string;
  activity: CodexSubagentActivityKind;
};

/**
 * 解析 Codex 原生子智能体活动参数。
 *
 * @param argumentsText 工具参数 JSON
 * @returns 子智能体活动；不是受支持的 Codex 活动时返回空
 */
export function parseCodexSubagentActivity(argumentsText: string): CodexSubagentActivity | null {
  const argumentsRecord = parseJsonRecord(argumentsText);
  if (!argumentsRecord) return null;
  const metadata = nestedRecord(argumentsRecord, ["_acp", "meta", "codex", "subagent"]);
  const threadId = stringField(argumentsRecord, "agentThreadId") || stringField(metadata, "threadId");
  const path = stringField(argumentsRecord, "agentPath") || stringField(metadata, "path");
  const activity = stringField(argumentsRecord, "activityKind") || stringField(metadata, "activity");
  if (!threadId || !path || !isActivityKind(activity)) return null;
  return {
    threadId,
    path,
    name: subagentName(path, threadId),
    activity
  };
}

/**
 * 沿指定字段路径读取嵌套对象。
 *
 * @param record 起始对象
 * @param path 字段路径
 * @returns 路径末端对象；任一层无效时返回空
 */
function nestedRecord(record: JsonRecord, path: readonly string[]): JsonRecord | null {
  let current: unknown = record;
  for (const key of path) {
    if (!current || typeof current !== "object" || Array.isArray(current)) return null;
    current = (current as JsonRecord)[key];
  }
  return current && typeof current === "object" && !Array.isArray(current)
    ? current as JsonRecord
    : null;
}

/**
 * 判断活动类型是否属于 Codex 当前公布的子智能体事件。
 *
 * @param value 活动类型文本
 * @returns 是否为受支持类型
 */
function isActivityKind(value: string): value is CodexSubagentActivityKind {
  return value === "started" || value === "interacted" || value === "interrupted";
}

/**
 * 从 agentPath 最后一段推导紧凑名称。
 *
 * @param path Codex agent 路径
 * @param threadId 线程标识回退值
 * @returns 子智能体展示名称
 */
function subagentName(path: string, threadId: string): string {
  const segments = path.split("/").map((segment) => segment.trim()).filter(Boolean);
  return segments.at(-1) ?? threadId;
}
