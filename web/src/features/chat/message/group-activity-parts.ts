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
  // 1. 【上下文】【压缩反馈】压缩结果独立展示，避免完成后收进普通工具折叠组
  if (part.type === "tool" && part.tool.name === "compress_context") return false;
  return part.type === "reasoning" || part.type === "tool" || part.type === "permission" || part.type === "ssh_secret";
}

/**
 * 判断部件是否为只含空白的正文片段。
 *
 * 部分模型（如 Gemini）在每轮思考与工具调用之间只输出换行，没有真正的正文。
 * 这类片段既不渲染，也不应把连续的工作组拆开。
 *
 * @param part 消息部件
 * @returns 只含空白时为 true
 */
export function isBlankText(part: LiveMessagePart): boolean {
  return part.type === "text" && !part.source.trim();
}

/**
 * 把连续的思考与工具收成正文前的工作组，组内相邻工具再收成一轮。
 *
 * @param parts 有序消息部件
 * @returns 可渲染的段落序列
 */
export function groupActivityParts(parts: LiveMessagePart[]): MessageSegment[] {
  // 1. 先丢掉空白正文，相邻的思考与工具才能连成一组，不留空白间隙
  parts = parts.filter((part) => !isBlankText(part));
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

/** 工具按用途的粗分类，用于折叠态摘要的「写入 N · 读取 M」式描述。 */
export type WorkItemCounts = {
  reasoning: number;
  tools: number;
  read: number;
  write: number;
  command: number;
  other: number;
};

/** 读类工具：查看文件、检索、搜索网络 */
const READ_TOOLS = new Set([
  "read_file",
  "grep",
  "glob",
  "web_fetch",
  "web_search",
  "ssh_list_hosts"
]);

/** 写类工具：新建、修改、删除文件 */
const WRITE_TOOLS = new Set([
  "write_file",
  "str_replace",
  "edit_file",
  "trash_path",
  "ssh_upload_file"
]);

/** 命令类工具：执行 shell、管理后台任务 */
const COMMAND_TOOLS = new Set([
  "run_command",
  "background_command",
  "ssh_run_command"
]);

/**
 * 按工具名归类为读 / 写 / 命令 / 其他。
 *
 * @param name 工具名
 * @returns 类别
 */
function classifyTool(name: string): "read" | "write" | "command" | "other" {
  if (isReadToolName(name)) return "read";
  if (WRITE_TOOLS.has(name)) return "write";
  if (COMMAND_TOOLS.has(name) || name.includes("background_command")) return "command";
  return "other";
}

/**
 * 判断工具是否属于读类。
 *
 * @param name 工具名
 * @returns 读类返回 true
 */
export function isReadToolName(name: string): boolean {
  return READ_TOOLS.has(name);
}

export type WaveRenderSegment =
  | { kind: "cards"; parts: WavePart[] }
  | { kind: "readFold"; parts: ToolPart[] };

/**
 * 判断读类工具是否已完成、可以提前收进紧凑组。
 *
 * 失败项默认展开，不并入折叠组。
 *
 * @param part 工作波次部件
 * @returns 可提前折叠时为 true
 */
function isFoldableRead(part: WavePart): part is ToolPart {
  return part.type === "tool"
    && isReadToolName(part.tool.name)
    && part.tool.status === "completed";
}

/**
 * 把连续已完成的读类工具提前收成紧凑组，其余仍逐条展示。
 *
 * 工作组展开时不要等整波结束才折叠：已完成的连续读取立刻收起，
 * 当前仍在执行的那一条继续展开。单独一条已完成读取、下一条读取仍在执行时也会提前收起。
 *
 * @param parts 一轮工具/权限部件
 * @returns 逐条卡片与提前折叠组交替排列
 */
export function splitWaveForEarlyReadFold(parts: readonly WavePart[]): WaveRenderSegment[] {
  const segments: WaveRenderSegment[] = [];
  let cards: WavePart[] = [];

  /**
   * 把累积的逐条卡片冲进结果。
   *
   * @returns 无
   */
  const flushCards = () => {
    if (cards.length === 0) return;
    segments.push({ kind: "cards", parts: cards });
    cards = [];
  };

  let index = 0;
  while (index < parts.length) {
    const part = parts[index];
    if (!isFoldableRead(part)) {
      cards.push(part);
      index += 1;
      continue;
    }
    // 1. 收集连续已完成读取
    const run: ToolPart[] = [];
    while (index < parts.length && isFoldableRead(parts[index])) {
      run.push(parts[index] as ToolPart);
      index += 1;
    }
    const next = parts[index];
    const nextIsActiveRead = next?.type === "tool"
      && isReadToolName(next.tool.name)
      && (next.tool.status === "preparing" || next.tool.status === "running");
    // 2. 两条及以上直接折叠；单独一条且下一条读取仍在执行时也提前收起
    if (run.length >= 2 || (run.length === 1 && nextIsActiveRead)) {
      flushCards();
      segments.push({ kind: "readFold", parts: run });
      continue;
    }
    cards.push(...run);
  }
  flushCards();
  return segments;
}

/**
 * 统计工作组里的思考段与工具调用数，并按用途拆分。
 *
 * @param items 工作组条目
 * @returns 思考段数、工具总数与按用途分类的计数
 */
export function countWorkItems(items: WorkItem[]): WorkItemCounts {
  const counts: WorkItemCounts = {
    reasoning: 0,
    tools: 0,
    read: 0,
    write: 0,
    command: 0,
    other: 0
  };
  for (const item of items) {
    if (item.kind === "reasoning") {
      counts.reasoning += 1;
      continue;
    }
    for (const part of item.parts) {
      if (part.type !== "tool") continue;
      counts.tools += 1;
      counts[classifyTool(part.tool.name)] += 1;
    }
  }
  return counts;
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
