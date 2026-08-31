/** 工具的三段可用状态 */
export type ToolMode = "on" | "load" | "off";

/** 延迟集合通配符：白名单内的全部非基础工具都需要 load */
export const DEFERRED_ALL_NON_BASE = "*";

/** Agent 档案中与工具权限相关的两个字段 */
export type ToolModeSelection = {
  /** 可用工具白名单，空数组表示全量开放 */
  enabled: string[];
  /** 其中需要模型调用 load 后才暴露的工具 */
  deferred: string[];
};

/**
 * 判定单个工具当前的三段状态。
 *
 * @param selection 当前启用与延迟集合
 * @param name 工具名称
 * @param isResident 该工具是否常驻（延迟集合含通配符时仍然直接可见）
 * @returns 工具的三段状态
 */
export function resolveToolMode(
  selection: ToolModeSelection,
  name: string,
  isResident: boolean
): ToolMode {
  const available = selection.enabled.length === 0 || selection.enabled.includes(name);
  if (!available) return "off";
  // 点名优先于通配符：用户显式选了按需就必须真的按需，常驻集合只是默认值
  if (selection.deferred.includes(name)) return "load";
  if (selection.deferred.includes(DEFERRED_ALL_NON_BASE) && !isResident) return "load";
  return "on";
}

/**
 * 批量更新一组工具的三段状态，并保持两个数组互斥且有序。
 *
 * 白名单为空代表全量开放，此时把某个工具切到 off 需要先把当前可见集合
 * 具体化为白名单，否则单个工具的关闭无法表达。
 *
 * @param selection 当前启用与延迟集合
 * @param names 本次需要更新的工具名称
 * @param mode 目标状态
 * @param allNames 全部可用工具名称，用于把「全量开放」具体化
 * @returns 更新后的启用与延迟集合
 */
export function updateToolModes(
  selection: ToolModeSelection,
  names: string[],
  mode: ToolMode,
  allNames: string[]
): ToolModeSelection {
  const targets = new Set(names);
  // 1. 关闭工具时，先把隐式的全量白名单展开成显式列表
  const baseEnabled = selection.enabled.length === 0 && mode === "off"
    ? [...allNames]
    : [...selection.enabled];

  const enabled = baseEnabled.filter((name, index) => baseEnabled.indexOf(name) === index && !targets.has(name));
  const deferred = selection.deferred.filter((name) => !targets.has(name));

  // 2. 通配符与逐项状态不能并存，逐项设置时把通配符展开为具体名称
  const hasWildcard = deferred.includes(DEFERRED_ALL_NON_BASE);
  if (mode !== "off") {
    for (const name of names) {
      if (!enabled.includes(name)) enabled.push(name);
    }
  }
  if (mode === "load") {
    for (const name of names) {
      if (!hasWildcard && !deferred.includes(name)) deferred.push(name);
    }
  }
  return { enabled, deferred };
}

/**
 * 展开延迟集合中的通配符，改写为逐项列出的具体工具名。
 *
 * 通配符便于表达默认策略，但用户一旦逐项调整就需要具体化，
 * 否则单个工具的 on 无法覆盖通配符。
 *
 * @param selection 当前启用与延迟集合
 * @param nonResidentNames 全部非常驻工具名称
 * @returns 不含通配符的延迟集合
 */
export function expandWildcard(
  selection: ToolModeSelection,
  nonResidentNames: string[]
): ToolModeSelection {
  if (!selection.deferred.includes(DEFERRED_ALL_NON_BASE)) return selection;
  const explicit = selection.deferred.filter((name) => name !== DEFERRED_ALL_NON_BASE);
  for (const name of nonResidentNames) {
    if (!explicit.includes(name)) explicit.push(name);
  }
  return { enabled: selection.enabled, deferred: explicit };
}

/**
 * 统计一组工具中各状态的数量。
 *
 * @param selection 当前启用与延迟集合
 * @param names 待统计的工具名称
 * @param isResident 判定工具是否常驻
 * @returns 三段状态各自的数量
 */
export function countToolModes(
  selection: ToolModeSelection,
  names: string[],
  isResident: (name: string) => boolean
): Record<ToolMode, number> {
  const counts: Record<ToolMode, number> = { on: 0, load: 0, off: 0 };
  for (const name of names) {
    counts[resolveToolMode(selection, name, isResident(name))] += 1;
  }
  return counts;
}
