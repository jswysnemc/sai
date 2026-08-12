/** 编辑器文件访问历史：栈 + 当前位置。 */
export type FileNavigationHistory = {
  stack: string[];
  index: number;
};

export const EMPTY_FILE_NAVIGATION_HISTORY: FileNavigationHistory = { stack: [], index: -1 };

/**
 * 记录一次文件访问。
 *
 * 与浏览器历史语义一致：在中段位置产生新访问会截断前进分支；
 * 与当前位置相同的路径不重复入栈。
 *
 * @param history 当前历史
 * @param path 访问的文件路径
 * @returns 新历史
 */
export function recordFileVisit(history: FileNavigationHistory, path: string): FileNavigationHistory {
  if (history.stack[history.index] === path) return history;
  const stack = [...history.stack.slice(0, history.index + 1), path];
  return { stack, index: stack.length - 1 };
}

/**
 * 是否可以后退。
 *
 * @param history 当前历史
 * @returns 存在上一条记录时为 true
 */
export function canGoBack(history: FileNavigationHistory): boolean {
  return history.index > 0;
}

/**
 * 是否可以前进。
 *
 * @param history 当前历史
 * @returns 存在下一条记录时为 true
 */
export function canGoForward(history: FileNavigationHistory): boolean {
  return history.index >= 0 && history.index < history.stack.length - 1;
}

/**
 * 后退一步。
 *
 * @param history 当前历史
 * @returns 新历史与目标路径；不可后退时为 null
 */
export function goBack(history: FileNavigationHistory): { history: FileNavigationHistory; path: string } | null {
  if (!canGoBack(history)) return null;
  const index = history.index - 1;
  return { history: { ...history, index }, path: history.stack[index] };
}

/**
 * 前进一步。
 *
 * @param history 当前历史
 * @returns 新历史与目标路径；不可前进时为 null
 */
export function goForward(history: FileNavigationHistory): { history: FileNavigationHistory; path: string } | null {
  if (!canGoForward(history)) return null;
  const index = history.index + 1;
  return { history: { ...history, index }, path: history.stack[index] };
}
