import type {
  SessionContextPrompt,
  SessionTimeline,
  SessionTimelineTurn,
  TimelineToolEntry,
  TimelineTurnMessage
} from "../../api/contracts";
import type { TrajectoryRecord, TrajectoryRecordKind } from "./trajectory-record";
import { summarizeContent, summarizeToolArguments } from "./trajectory-format";

/** 构建产物：扁平记录表与按轮次归并的请求边界。 */
export type TrajectoryModel = {
  records: TrajectoryRecord[];
  /** 每个轮次的展示序号与总耗时，用于表格的轮次分隔行 */
  turns: TrajectoryTurnHeader[];
};

/** 表格中一条轮次分隔行所需的数据。 */
export type TrajectoryTurnHeader = {
  turnId: string;
  seq: number;
  status: SessionTimelineTurn["status"];
  durationMs: number | null;
  model: string | null;
  /** 本轮发出的模型请求次数 */
  requestCount: number;
  /** 本轮记录在 records 中的起始下标 */
  firstIndex: number;
};

/**
 * 把会话时间线折叠成按时序排列的轨迹记录。
 *
 * 轮内顺序按模型实际经历的次序还原：用户输入在前，随后是每次模型请求
 * 产生的工具批次，插入消息紧跟在触发它的工具之后，助手正文收尾。
 *
 * 系统提示词排在最前：它是每次请求都会重发的前缀，也是上下文占用的
 * 大头，轨迹里缺了它就看不出一次任务的固定成本从哪来。
 *
 * @param timeline 会话时间线响应
 * @param contextPrompt 当前会话的系统提示词快照
 * @returns 扁平记录表与轮次分隔数据
 */
export function buildTrajectory(
  timeline: SessionTimeline | undefined,
  contextPrompt?: SessionContextPrompt
): TrajectoryModel {
  const records: TrajectoryRecord[] = [];
  const turns: TrajectoryTurnHeader[] = [];
  if (contextPrompt?.content?.trim()) {
    records.push(systemRecord(contextPrompt));
  }
  if (!timeline) return { records, turns };

  for (const turn of timeline.turns) {
    const firstIndex = records.length;
    const rounds = groupToolsByRound(turn.tools ?? []);
    const roundOf = new Map<number, number>();
    rounds.forEach((group, position) => roundOf.set(group.round, position + 1));
    // 助手正文来自最后一批工具之后的那次请求；没有工具时就是首次请求
    const finalRound = rounds.length + 1;
    let started = false;

    /**
     * 追加一条记录并维护轮次与请求边界标记。
     *
     * @param record 除序号与边界标记外已填好的记录
     * @returns 无
     */
    const push = (record: Omit<TrajectoryRecord, "index" | "turnStart" | "roundStart">) => {
      const previous = records[records.length - 1];
      records.push({
        ...record,
        index: records.length + 1,
        turnStart: !started,
        roundStart: previous?.turnId !== record.turnId || previous.round !== record.round
      });
      started = true;
    };

    push({
      id: `${turn.turn_id}/user`,
      kind: "user",
      turnId: turn.turn_id,
      turnSeq: turn.seq,
      round: 0,
      summary: summarizeContent(turn.user.content),
      label: turn.automatic ? "auto" : null,
      startedAt: parseTimestamp(turn.user.timestamp),
      durationMs: null,
      failed: false,
      running: false,
      detail: {
        input: turn.user.content,
        imageUrls: turn.user.image_urls ?? [],
        usage: turn.usage ?? null,
        model: turn.model ?? null
      }
    });

    const messages = [...(turn.messages ?? [])].sort((left, right) => left.seq - right.seq);
    pushMessagesAfter(messages, 0, turn, 1, push);

    for (const group of rounds) {
      const round = roundOf.get(group.round) ?? 1;
      for (const tool of group.tools) {
        push(toolRecord(tool, turn, round));
        pushMessagesAfter(messages, tool.seq ?? 0, turn, round, push);
      }
    }

    if (turn.assistant.content.trim() || turn.assistant.reasoning?.trim() || turn.error) {
      push({
        id: `${turn.turn_id}/assistant`,
        kind: "assistant",
        turnId: turn.turn_id,
        turnSeq: turn.seq,
        round: finalRound,
        summary: summarizeContent(turn.assistant.content || turn.error || ""),
        label: turn.model ?? null,
        startedAt: parseTimestamp(turn.assistant.timestamp),
        durationMs: null,
        failed: Boolean(turn.error),
        running: turn.status === "running",
        detail: {
          input: turn.assistant.content,
          reasoning: turn.assistant.reasoning ?? undefined,
          error: turn.error ?? undefined,
          imageUrls: turn.assistant.image_urls ?? [],
          usage: turn.usage ?? null,
          model: turn.model ?? null
        }
      });
    }

    if (records.length > firstIndex) {
      turns.push({
        turnId: turn.turn_id,
        seq: turn.seq,
        status: turn.status,
        durationMs: turn.duration_ms ?? null,
        model: turn.model ?? null,
        requestCount: rounds.length + 1,
        firstIndex
      });
    }
  }

  const compaction = timeline.compaction;
  if (compaction?.summary?.trim()) {
    records.push({
      id: "compaction",
      index: records.length + 1,
      kind: "compaction",
      turnId: null,
      turnSeq: null,
      turnStart: true,
      round: 0,
      roundStart: true,
      summary: summarizeContent(compaction.summary),
      label: compaction.reason,
      startedAt: parseTimestamp(compaction.created_at),
      durationMs: null,
      failed: false,
      running: false,
      detail: { input: compaction.summary }
    });
  }

  return { records, turns };
}

/**
 * 把系统提示词快照转换为轨迹首条记录。
 *
 * 摘要给出体量与构成而不是正文开头：这条记录的价值在于"固定成本多大、
 * 由哪些部分组成"，正文本身在详情里按分区读。
 *
 * @param prompt 系统提示词快照
 * @returns 系统记录
 */
function systemRecord(prompt: SessionContextPrompt): TrajectoryRecord {
  const parts: string[] = [];
  if (prompt.token_count) parts.push(`${prompt.token_count} tokens`);
  else if (prompt.char_count) parts.push(`${prompt.char_count} chars`);
  if (prompt.tool_count) parts.push(`${prompt.tool_count} tools`);
  for (const section of prompt.sections ?? []) parts.push(section.label);
  return {
    id: "system-prompt",
    index: 1,
    kind: "system",
    turnId: null,
    turnSeq: null,
    turnStart: false,
    round: 0,
    roundStart: false,
    summary: parts.join(" · "),
    label: prompt.source === "live" ? "live" : "baseline",
    startedAt: null,
    durationMs: null,
    failed: false,
    running: false,
    detail: {
      input: prompt.content,
      sections: prompt.sections
    }
  };
}

/**
 * 把插入消息追加到指定工具之后。
 *
 * @param messages 已按序号排序的轮内消息
 * @param afterToolSeq 触发点的工具序号；0 表示位于所有工具之前
 * @param turn 所属轮次
 * @param round 当前模型请求序号
 * @param push 记录追加函数
 * @returns 无
 */
function pushMessagesAfter(
  messages: TimelineTurnMessage[],
  afterToolSeq: number,
  turn: SessionTimelineTurn,
  round: number,
  push: (record: Omit<TrajectoryRecord, "index" | "turnStart" | "roundStart">) => void
): void {
  for (const message of messages) {
    if ((message.after_tool_seq ?? 0) !== afterToolSeq) continue;
    push({
      id: `${turn.turn_id}/message/${message.id}`,
      kind: "message",
      turnId: turn.turn_id,
      turnSeq: turn.seq,
      round,
      summary: summarizeContent(message.content),
      label: message.kind,
      startedAt: parseTimestamp(message.created_at),
      durationMs: null,
      failed: false,
      running: false,
      detail: {
        input: message.content,
        reasoning: message.reasoning ?? undefined,
        imageUrls: message.image_urls ?? []
      }
    });
  }
}

/**
 * 把一次工具调用转换为轨迹记录。
 *
 * @param tool 时间线中的工具条目
 * @param turn 所属轮次
 * @param round 当前模型请求序号
 * @returns 待追加的记录
 */
function toolRecord(
  tool: TimelineToolEntry,
  turn: SessionTimelineTurn,
  round: number
): Omit<TrajectoryRecord, "index" | "turnStart" | "roundStart"> {
  const startedAt = parseTimestamp(tool.created_at);
  const completedAt = tool.completed_at ? parseTimestamp(tool.completed_at) : null;
  return {
    id: `${turn.turn_id}/tool/${tool.id}`,
    kind: "tool" satisfies TrajectoryRecordKind,
    turnId: turn.turn_id,
    turnSeq: turn.seq,
    round,
    summary: summarizeToolArguments(tool.arguments),
    label: tool.name,
    startedAt,
    durationMs: startedAt != null && completedAt != null ? Math.max(0, completedAt - startedAt) : null,
    failed: tool.status === "failed" || tool.ok === false,
    running: tool.status === "running",
    detail: {
      input: tool.arguments,
      inputIsJson: true,
      output: tool.output,
      error: tool.error ?? undefined,
      originalChars: tool.original_chars ?? null,
      resultRef: tool.result_ref ?? null,
      permission: tool.permission ?? null,
      usage: turn.usage ?? null,
      model: turn.model ?? null
    }
  };
}

/** 同一次模型请求产生的一批工具调用。 */
type ToolRoundGroup = { round: number; tools: TimelineToolEntry[] };

/**
 * 按模型子轮编号把工具调用分批。
 *
 * 后端的 assistant_round 是批次标识而非连续编号，这里保持它的分组语义，
 * 连续的请求序号由调用方按批次顺序另行编号。
 *
 * @param tools 轮内全部工具调用
 * @returns 按首次出现顺序排列的工具批次
 */
function groupToolsByRound(tools: TimelineToolEntry[]): ToolRoundGroup[] {
  const ordered = [...tools].sort((left, right) => (left.seq ?? 0) - (right.seq ?? 0));
  const groups: ToolRoundGroup[] = [];
  for (const tool of ordered) {
    const round = tool.assistant_round ?? tool.seq ?? 0;
    const current = groups[groups.length - 1];
    if (current && current.round === round) current.tools.push(tool);
    else groups.push({ round, tools: [tool] });
  }
  return groups;
}

/**
 * 解析后端时间戳为毫秒数。
 *
 * @param value ISO 时间戳文本
 * @returns 毫秒时间戳；无法解析时返回 null
 */
function parseTimestamp(value: string | null | undefined): number | null {
  if (!value) return null;
  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? parsed : null;
}
