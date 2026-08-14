import type { SubagentDetail, SubagentTimelineEntry } from "../../api/contracts";
import { summarizeContent, summarizeToolArguments } from "./trajectory-format";
import type { TrajectoryRecord } from "./trajectory-record";

/**
 * 从 subagent 工具的输出里取出子智能体标识。
 *
 * 工具返回 `{ ok, subagent: { id, … }, message }`，标识就在其中。
 * 输出预览可能被截断，因而解析失败是常态，不作为错误处理。
 *
 * @param output 工具输出文本
 * @returns 子智能体标识；无法解析时返回 null
 */
export function subagentIdFromOutput(output: string | undefined): string | null {
  if (!output?.trim()) return null;
  try {
    const parsed: unknown = JSON.parse(output);
    if (parsed === null || typeof parsed !== "object") return null;
    const subagent = (parsed as { subagent?: { id?: unknown } }).subagent;
    return typeof subagent?.id === "string" && subagent.id ? subagent.id : null;
  } catch {
    // 截断的预览仍可能残留 "id": "…"，退回文本匹配
    const matched = /"id"\s*:\s*"([^"]+)"/.exec(output);
    return matched?.[1] ?? null;
  }
}

/**
 * 收集轨迹里全部被引用的子智能体标识。
 *
 * @param records 轨迹记录
 * @returns 去重后的子智能体标识
 */
export function referencedSubagentIds(records: readonly TrajectoryRecord[]): string[] {
  const ids = new Set<string>();
  for (const record of records) {
    if (record.kind !== "tool" || record.label !== "subagent") continue;
    const id = subagentIdFromOutput(record.detail.output);
    if (id) ids.add(id);
  }
  return [...ids];
}

/**
 * 把子智能体的时间线展开为轨迹记录。
 *
 * 这些条目只有步号没有时刻，因此不带 startedAt——概览按真实时间投影，
 * 给它们编一个时刻会让子智能体凭空占据一段并不存在的耗时。
 *
 * @param detail 子智能体详情
 * @param parentId 触发它的工具记录标识
 * @returns 子智能体记录
 */
export function subagentRecords(
  detail: SubagentDetail,
  parentId: string
): Array<Omit<TrajectoryRecord, "index">> {
  return detail.timeline.map((entry, position) => ({
    id: `${parentId}/sub/${position}`,
    kind: "subagent" as const,
    turnId: null,
    turnSeq: null,
    turnStart: false,
    round: 0,
    roundStart: false,
    summary: entrySummary(entry),
    label: entryLabel(entry, detail),
    startedAt: null,
    durationMs: null,
    failed: entry.kind === "tool" && entry.ok === false,
    running: false,
    parentId,
    detail: entryDetail(entry)
  }));
}

/**
 * 生成子智能体条目的单行摘要。
 *
 * @param entry 时间线条目
 * @returns 摘要文本
 */
function entrySummary(entry: SubagentTimelineEntry): string {
  switch (entry.kind) {
    case "tool":
      return summarizeToolArguments(entry.args_preview);
    case "message":
      return summarizeContent(entry.text);
    default:
      return summarizeContent(entry.text);
  }
}

/**
 * 生成子智能体条目的次级标签。
 *
 * @param entry 时间线条目
 * @param detail 所属子智能体
 * @returns 标签文本
 */
function entryLabel(entry: SubagentTimelineEntry, detail: SubagentDetail): string {
  switch (entry.kind) {
    case "tool":
      return entry.name;
    case "message":
      return entry.from;
    case "reasoning":
      return `${detail.subagent_type} · reasoning`;
    default:
      return detail.subagent_type;
  }
}

/**
 * 生成子智能体条目的详情内容。
 *
 * @param entry 时间线条目
 * @returns 详情
 */
function entryDetail(entry: SubagentTimelineEntry): TrajectoryRecord["detail"] {
  if (entry.kind === "tool") {
    return {
      input: entry.args_preview,
      inputIsJson: true,
      output: entry.output_preview ?? undefined
    };
  }
  if (entry.kind === "reasoning") return { reasoning: entry.text };
  return { input: entry.text };
}
