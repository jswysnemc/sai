const ADJECTIVES = [
  "bright",
  "calm",
  "clear",
  "focused",
  "rapid",
  "steady",
  "tidy",
  "vivid"
] as const;

const NOUNS = [
  "anchor",
  "bridge",
  "cedar",
  "delta",
  "harbor",
  "ridge",
  "signal",
  "summit"
] as const;

/**
 * 生成符合 Git ref 规则的紧凑分支名称建议。
 *
 * @param random 可注入的零到一随机数函数
 * @returns 形如 bright-cedar-k4m 的分支名称
 */
export function createBranchNameSuggestion(random: () => number = Math.random): string {
  // 1. 单独选择两个单词，便于用户快速识别和修改
  const adjective = ADJECTIVES[randomIndex(ADJECTIVES.length, random)];
  const noun = NOUNS[randomIndex(NOUNS.length, random)];
  // 2. 添加三位基础三十六进制后缀，降低并行创建时的碰撞概率
  const suffix = Math.floor(normalizeRandom(random()) * 46_656)
    .toString(36)
    .padStart(3, "0");
  return `${adjective}-${noun}-${suffix}`;
}

/**
 * 将随机值转换为数组有效下标。
 *
 * @param length 数组长度
 * @param random 随机数函数
 * @returns 有效数组下标
 */
function randomIndex(length: number, random: () => number): number {
  return Math.floor(normalizeRandom(random()) * length);
}

/**
 * 将异常随机值限制到零到一的左闭右开区间。
 *
 * @param value 原始随机值
 * @returns 可用于下标计算的随机值
 */
function normalizeRandom(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.min(0.999999999, Math.max(0, value));
}
