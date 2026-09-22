/** 左栏三种浏览：会话、工作区、文件树。 */
export type SidebarBrowseMode = "sessions" | "workspaces" | "files";

const MODE_KEY = "sai.sidebar.browse-mode";
const RECENTS_KEY = "sai.sidebar.recents";

/** 默认展开的最近条数，其余收进历史。 */
export const SIDEBAR_RECENT_LIMIT = 3;

export type SidebarRecents = {
  sessions: string[];
  workspaces: string[];
  /** 打开左栏时看到的会话，之后不再随排序变动。 */
  sessionBaseline: string[];
  /** 打开左栏时看到的工作区，之后不再随排序变动。 */
  workspaceBaseline: string[];
};

/**
 * 读取上次停留的左栏浏览。
 *
 * @returns 会话、工作区或文件树
 */
export function readBrowseMode(): SidebarBrowseMode {
  const value = localStorage.getItem(MODE_KEY);
  return value === "workspaces" || value === "files" ? value : "sessions";
}

/**
 * 记住左栏浏览。
 *
 * @param mode 目标浏览
 */
export function writeBrowseMode(mode: SidebarBrowseMode) {
  localStorage.setItem(MODE_KEY, mode);
}

/**
 * 读取最近激活顺序。读失败时当作还没有记录。
 *
 * @returns 会话和工作区标识，新的在前
 */
export function readRecents(): SidebarRecents {
  try {
    const parsed = JSON.parse(localStorage.getItem(RECENTS_KEY) ?? "") as Partial<SidebarRecents>;
    return {
      sessions: stringList(parsed.sessions),
      workspaces: stringList(parsed.workspaces),
      sessionBaseline: stringList(parsed.sessionBaseline),
      workspaceBaseline: stringList(parsed.workspaceBaseline)
    };
  } catch {
    return { sessions: [], workspaces: [], sessionBaseline: [], workspaceBaseline: [] };
  }
}

/**
 * 把刚激活的会话或工作区放到最近列表最前。
 *
 * @param kind 会话或工作区
 * @param id 标识
 * @returns 写回后的完整顺序
 */
export function rememberRecent(kind: "sessions" | "workspaces", id: string): SidebarRecents {
  const current = readRecents();
  const next = { ...current, [kind]: [id, ...current[kind].filter((item) => item !== id)] };
  localStorage.setItem(RECENTS_KEY, JSON.stringify(next));
  return next;
}

/**
 * 记下打开时直接展示的那几条。已经记过就保持不变。
 *
 * @param kind 会话或工作区的初始名单
 * @param ids 当前按最近程度排好的标识
 * @returns 需要写回的状态；初始名单已存在时返回 null
 */
export function captureRecentBaseline(kind: "sessionBaseline" | "workspaceBaseline", ids: string[]): SidebarRecents | null {
  const current = readRecents();
  if (current[kind].length > 0 || ids.length === 0) return null;
  const next = { ...current, [kind]: ids.slice(0, SIDEBAR_RECENT_LIMIT) };
  localStorage.setItem(RECENTS_KEY, JSON.stringify(next));
  return next;
}

/**
 * 默认展示打开时的前几条，并保留之后被激活过的条目。
 *
 * 初始名单还没记下时，沿用当前顺序的前几条。记下之后，激活历史条目只追加，
 * 排序变化不会把原来的最近一条挤回历史。
 *
 * @param items 全部条目，调用方按最近程度排好
 * @param idOf 取标识
 * @param activated 已激活顺序，新的在前
 * @param baseline 打开时固定的标识；空则退回当前顺序的前几条
 * @param limit 默认直接展示的条数
 * @returns 最近条目和历史条目
 */
export function splitByRecent<T>(items: T[], idOf: (item: T) => string, activated: string[], baseline: string[] = [], limit = SIDEBAR_RECENT_LIMIT): { recent: T[]; history: T[] } {
  const byId = new Map(items.map((item) => [idOf(item), item]));
  const seen = new Set<string>();
  const recent: T[] = [];
  const take = (item: T | undefined) => {
    if (!item) return;
    const id = idOf(item);
    if (seen.has(id)) return;
    seen.add(id);
    recent.push(item);
  };
  for (const id of activated) take(byId.get(id));
  const window = baseline.length > 0 ? baseline.map((id) => byId.get(id)) : items.slice(0, limit);
  for (const item of window) take(item);
  return { recent, history: items.filter((item) => !seen.has(idOf(item))) };
}

/**
 * 只保留字符串标识。
 *
 * @param value 可能不是数组的存储值
 * @returns 字符串列表
 */
function stringList(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
}
