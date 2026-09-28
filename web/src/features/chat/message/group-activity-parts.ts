import type { LiveMessagePart } from "../run-event-reducer";

export type ReasoningPart = Extract<LiveMessagePart, { type: "reasoning" }>;
export type ToolPart = Extract<LiveMessagePart, { type: "tool" }>;
export type PermissionPart = Extract<LiveMessagePart, { type: "permission" }>;
export type SshSecretPart = Extract<LiveMessagePart, { type: "ssh_secret" }>;
export type WavePart = ToolPart | PermissionPart | SshSecretPart;

export type WorkItem =
  | { kind: "reasoning"; part: ReasoningPart }
  | { kind: "wave"; parts: WavePart[] };

export type MessageSegment =
  | { type: "preamble"; id: string; items: WorkItem[]; followedByText: boolean }
  | { type: "part"; part: LiveMessagePart };

/**
 * 判断部件是否属于正文前的工作流（思考、工具、获批权限、SSH 安全输入）。
 *
 * @param part 消息部件
 * @returns 是否为可编组的工作部件
 */
export function isWorkPart(part: LiveMessagePart): part is ReasoningPart | WavePart {
  return part.type === "reasoning" || part.type === "tool" || part.type === "permission" || part.type === "ssh_secret";
}

/**
 * 把连续的思考与工具收成正文前的工作组，组内相邻工具再收成一轮。
 *
 * @param parts 有序消息部件
 * @returns 可渲染的段落序列
 */
export function groupActivityParts(parts: LiveMessagePart[]): MessageSegment[] {
  const segments: MessageSegment[] = [];
  let index = 0;
  while (index < parts.length) {
    const part = parts[index];
    if (!isWorkPart(part)) {
      segments.push({ type: "part", part });
      index += 1;
      continue;
    }
    const work: Array<ReasoningPart | WavePart> = [];
    while (index < parts.length && isWorkPart(parts[index])) {
      work.push(parts[index] as ReasoningPart | WavePart);
      index += 1;
    }
    segments.push({
      type: "preamble",
      id: `preamble-${work[0].id}-${work[work.length - 1].id}`,
      items: clusterWorkItems(work),
      followedByText: parts[index]?.type === "text"
    });
  }
  return segments;
}

/**
 * 统计工作组里的思考段与工具调用数。
 *
 * @param items 工作组条目
 * @returns 思考段数与工具数
 */
export function countWorkItems(items: WorkItem[]): { reasoning: number; tools: number } {
  let reasoning = 0;
  let tools = 0;
  for (const item of items) {
    if (item.kind === "reasoning") {
      reasoning += 1;
      continue;
    }
    tools += item.parts.filter((part) => part.type === "tool").length;
  }
  return { reasoning, tools };
}

/**
 * 取出工作组里按出现顺序排列的工具调用。
 *
 * @param items 工作组条目
 * @returns 工具部件
 */
export function collectWaveTools(items: WorkItem[]): ToolPart[] {
  return items.flatMap((item) => (
    item.kind === "wave" ? item.parts.filter((part): part is ToolPart => part.type === "tool") : []
  ));
}

/**
 * 取出工作组里的权限请求卡，折叠态仍需展示拒绝决定。
 *
 * @param items 工作组条目
 * @returns 权限部件
 */
export function collectWavePermissions(items: WorkItem[]): PermissionPart[] {
  return items.flatMap((item) => (
    item.kind === "wave" ? item.parts.filter((part): part is PermissionPart => part.type === "permission") : []
  ));
}

/**
 * 取出工作组里的 SSH 安全输入卡，折叠态仍需展示以免用户看不见密码框。
 *
 * @param items 工作组条目
 * @returns SSH 安全输入部件
 */
export function collectWaveSecrets(items: WorkItem[]): SshSecretPart[] {
  return items.flatMap((item) => (
    item.kind === "wave" ? item.parts.filter((part): part is SshSecretPart => part.type === "ssh_secret") : []
  ));
}

/**
 * 把连续工具（含夹在中间的权限卡）收成一轮，思考单独成项。
 *
 * @param parts 一段连续工作部件
 * @returns 思考项与工具轮
 */
function clusterWorkItems(parts: Array<ReasoningPart | WavePart>): WorkItem[] {
  const items: WorkItem[] = [];
  let wave: WavePart[] = [];
  const flushWave = () => {
    if (wave.length === 0) return;
    items.push({ kind: "wave", parts: wave });
    wave = [];
  };
  for (const part of parts) {
    if (part.type === "reasoning") {
      flushWave();
      items.push({ kind: "reasoning", part });
      continue;
    }
    wave.push(part);
  }
  flushWave();
  return items;
}
