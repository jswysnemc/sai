import type { PermissionDecision, TurnUsage } from "../../api/contracts";

/** 轨迹记录的种类；决定行标签与配色。 */
export type TrajectoryRecordKind =
  | "system"
  | "user"
  | "thinking"
  | "assistant"
  | "tool"
  | "subagent"
  | "message"
  | "compaction";

/** 一条轨迹记录的完整数据。 */
export type TrajectoryRecord = {
  /** 跨重建稳定的标识，用于选中态与 React key */
  id: string;
  /** 从 1 开始的展示序号 */
  index: number;
  kind: TrajectoryRecordKind;
  /** 所属轮次标识；压缩记录不属于任何轮次时为 null */
  turnId: string | null;
  /** 轮次展示序号；压缩记录为 null */
  turnSeq: number | null;
  /** 是否为所属轮次的首条记录 */
  turnStart: boolean;
  /** 产生该记录的模型子轮编号；用于划分请求边界 */
  round: number;
  /** 是否为一次新模型请求的首条记录 */
  roundStart: boolean;
  /** 单行摘要文本 */
  summary: string;
  /** 工具名或消息种类等次级标签 */
  label: string | null;
  /** 起始时刻的毫秒时间戳；无法解析时为 null */
  startedAt: number | null;
  /** 自身耗时毫秒；无法计算时为 null */
  durationMs: number | null;
  /** 该记录是否处于失败态 */
  failed: boolean;
  /** 该记录是否仍在运行 */
  running: boolean;
  /** 触发该记录的父记录标识；子智能体条目据此缩进 */
  parentId?: string;
  /** 详情面板需要的完整内容 */
  detail: TrajectoryRecordDetail;
};

/** 展开详情时才使用的完整内容，列表渲染不读取。 */
export type TrajectoryRecordDetail = {
  /** 请求侧完整内容：用户输入、助手正文或工具入参 */
  input?: string;
  /** 入参是否为 JSON，决定详情面板是否格式化 */
  inputIsJson?: boolean;
  /** 响应侧完整内容：工具输出 */
  output?: string;
  /** 模型思考过程 */
  reasoning?: string;
  /** 失败原因 */
  error?: string;
  /** 所属轮次的汇总用量；同轮多条记录共享同一份 */
  usage?: TurnUsage | null;
  /** 本轮使用的模型标识 */
  model?: string | null;
  /** 工具输出被截断前的原始字符数 */
  originalChars?: number | null;
  /** 完整输出的外部引用 */
  resultRef?: string | null;
  /** 工具调用的权限裁决 */
  permission?: PermissionDecision | null;
  /** 附带的图片地址 */
  imageUrls?: string[];
  /** 系统提示词按来源拆分的分区，供详情面板分段展示 */
  sections?: Array<{ id: string; label: string; content: string }>;
  /** 是否直接来自真实供应商请求体 */
  actualRequest?: boolean;
  /** 是否为当前配置即时预览，而非历史真实请求 */
  preview?: boolean;
  /** 压缩摘要覆盖的起始轮次序号 */
  compactedFromSeq?: number | null;
  /** 压缩摘要覆盖的结束轮次序号 */
  compactedToSeq?: number | null;
};

/** 记录种类到界面标签的映射所需的双语文案对。 */
export type RecordKindLabel = { en: string; zh: string };

/** 各记录种类的短标签。 */
export const RECORD_KIND_LABELS: Record<TrajectoryRecordKind, RecordKindLabel> = {
  system: { en: "System", zh: "系统" },
  user: { en: "User", zh: "用户" },
  thinking: { en: "Think", zh: "思考" },
  assistant: { en: "Model", zh: "模型" },
  tool: { en: "Tool", zh: "工具" },
  subagent: { en: "Sub", zh: "子体" },
  message: { en: "Inserted", zh: "插入" },
  compaction: { en: "Compact", zh: "压缩" }
};

/**
 * 计算一条记录的结束时刻。
 *
 * @param record 轨迹记录
 * @returns 结束时刻毫秒；缺少起始时刻时返回 null，缺少耗时时退化为起始时刻
 */
export function recordEndedAt(record: TrajectoryRecord): number | null {
  if (record.startedAt == null) return null;
  return record.startedAt + (record.durationMs ?? 0);
}

/**
 * 判断记录是否落在给定的闭区间内。
 *
 * 与区间有任何重叠即算命中：只按起点判断会漏掉跨越区间边界的长工具。
 *
 * @param record 轨迹记录
 * @param from 区间起点毫秒
 * @param to 区间终点毫秒
 * @returns 是否与区间重叠
 */
export function recordOverlaps(record: TrajectoryRecord, from: number, to: number): boolean {
  if (record.startedAt == null) return false;
  const end = recordEndedAt(record) ?? record.startedAt;
  return record.startedAt <= to && end >= from;
}
