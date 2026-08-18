/**
 * 自动压缩触发点计算。
 *
 * 口径与后端 src/state/compaction/budget.rs 的 CompactionBudgetPolicy::trigger_chars
 * 保持一致：比例与预留是两个并列条件，取「更晚到达」的那个。小窗口通常比例先到，
 * 大窗口通常预留先到。任何一边改了都要同步另一边，否则面板预览会与实际行为不符。
 */

/** 允许配置的压缩比例下限（百分比） */
export const MIN_RATIO_PERCENT = 50;

/** 允许配置的压缩比例上限（百分比） */
export const MAX_RATIO_PERCENT = 99;

/** 内置默认压缩比例（百分比） */
export const DEFAULT_RATIO_PERCENT = 90;

/** 内置默认预留 token */
export const DEFAULT_RESERVE_TOKENS = 50_000;

/** 预留输入的快捷档，0 表示关闭预留条件、只按比例触发 */
export const RESERVE_PRESETS = [0, 8_000, 50_000, 100_000] as const;

/** 实际决定触发点的那个条件 */
export type ActiveTriggerCondition = "ratio" | "reserve";

export type TriggerBreakdown = {
  /** 最终触发点，两个条件取更晚到达者 */
  trigger: number;
  /** 只看比例条件时的触发点 */
  ratioTrigger: number;
  /** 只看预留条件时的触发点；预留关闭或超出窗口时为 null，表示该条件不参与 */
  reserveTrigger: number | null;
  /** 实际生效的条件 */
  active: ActiveTriggerCondition;
};

/**
 * 计算自动压缩触发点。
 *
 * @param windowTokens 当前模型的上下文窗口
 * @param ratio 压缩比例，0~1
 * @param reserveTokens 预留 token，0 表示只按比例
 * @returns 触发压缩的 token 数；窗口未知时返回 0
 */
export function computeTriggerTokens(windowTokens: number, ratio: number, reserveTokens: number): number {
  return resolveTriggerBreakdown(windowTokens, ratio, reserveTokens).trigger;
}

/**
 * 拆解触发点，给出两个条件各自的结果与实际生效者。
 *
 * 面板要让「为什么是这个触发点」可见，所以除了最终值还要保留中间量。
 *
 * @param windowTokens 当前模型的上下文窗口
 * @param ratio 压缩比例，0~1
 * @param reserveTokens 预留 token，0 表示只按比例
 * @returns 触发点拆解结果
 */
export function resolveTriggerBreakdown(
  windowTokens: number,
  ratio: number,
  reserveTokens: number
): TriggerBreakdown {
  // 1. 窗口未知时无从计算，各项归零
  if (windowTokens <= 0) {
    return { trigger: 0, ratioTrigger: 0, reserveTrigger: null, active: "ratio" };
  }

  // 2. 比例条件：占用达到窗口的该比例即触发
  const ratioTrigger = Math.max(1, Math.trunc(Math.max(1, windowTokens * ratio)));

  // 3. 预留为 0 或大到吃掉整个窗口时，该条件不参与，退回纯比例
  if (reserveTokens <= 0 || reserveTokens >= windowTokens) {
    return { trigger: ratioTrigger, ratioTrigger, reserveTrigger: null, active: "ratio" };
  }

  // 4. 预留条件：窗口剩余不足预留量即触发
  const reserveTrigger = windowTokens - reserveTokens;

  // 5. 取更晚到达者，相等时归给比例，与后端 max 的语义一致
  const trigger = Math.max(ratioTrigger, reserveTrigger);
  return {
    trigger,
    ratioTrigger,
    reserveTrigger,
    active: reserveTrigger > ratioTrigger ? "reserve" : "ratio"
  };
}

/**
 * 把比例百分比夹紧到合法区间。
 *
 * @param percent 用户输入的百分比
 * @returns 50~99 之间的整数百分比
 */
export function clampRatioPercent(percent: number): number {
  if (!Number.isFinite(percent)) return DEFAULT_RATIO_PERCENT;
  return Math.min(MAX_RATIO_PERCENT, Math.max(MIN_RATIO_PERCENT, Math.round(percent)));
}

/**
 * 把预留 token 夹紧到合法区间。
 *
 * 上限留在窗口之内：填满整个窗口等于让预留条件失效，不如直接引导用户关掉。
 *
 * @param tokens 用户输入的预留量
 * @param windowTokens 当前上下文窗口，未知时不设上限
 * @returns 不小于 0 的整数预留量
 */
export function clampReserveTokens(tokens: number, windowTokens: number): number {
  if (!Number.isFinite(tokens) || tokens <= 0) return 0;
  const rounded = Math.round(tokens);
  if (windowTokens > 0) return Math.min(rounded, windowTokens);
  return rounded;
}

/**
 * 解析预留输入，接受 50000、50k、50K 三种写法。
 *
 * 预留量常常是几万这个量级，逐位敲零既慢又容易多敲一位，因此放开 k 后缀。
 *
 * @param raw 输入框原始文本
 * @returns 解析出的 token 数；无法解析时为 null
 */
export function parseReserveInput(raw: string): number | null {
  const text = raw.trim().toLowerCase().replace(/[,_\s]/g, "");
  if (text === "") return 0;
  const match = /^(\d+(?:\.\d+)?)(k|m)?$/.exec(text);
  if (!match) return null;
  const value = Number(match[1]);
  if (!Number.isFinite(value)) return null;
  if (match[2] === "k") return Math.round(value * 1_000);
  if (match[2] === "m") return Math.round(value * 1_000_000);
  return Math.round(value);
}
