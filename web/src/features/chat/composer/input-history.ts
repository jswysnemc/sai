export type InputHistoryState = {
  index: number | null;
  draft: string;
};

export type InputHistoryResult = {
  state: InputHistoryState;
  value: string;
};

/**
 * 判断光标是否位于输入框第一行。
 *
 * @param value 输入文本
 * @param selectionStart 选区起点
 * @returns 是否位于第一行
 */
export function isCursorOnFirstLine(value: string, selectionStart: number): boolean {
  return !value.slice(0, selectionStart).includes("\n");
}

/**
 * 判断光标是否位于输入框最后一行。
 *
 * @param value 输入文本
 * @param selectionEnd 选区终点
 * @returns 是否位于最后一行
 */
export function isCursorOnLastLine(value: string, selectionEnd: number): boolean {
  return !value.slice(selectionEnd).includes("\n");
}

/**
 * 按方向在用户输入历史中移动，并在末尾恢复原草稿。
 *
 * @param entries 按发送顺序排列的用户输入
 * @param current 当前历史游标状态
 * @param value 当前草稿文本
 * @param direction 导航方向
 * @returns 新历史状态和输入值
 */
export function navigateInputHistory(
  entries: string[],
  current: InputHistoryState,
  value: string,
  direction: "up" | "down"
): InputHistoryResult | null {
  if (entries.length === 0) return null;
  if (direction === "up") {
    const initialIndex = current.index ?? entries.length;
    const index = Math.max(0, initialIndex - 1);
    return {
      state: { index, draft: current.index === null ? value : current.draft },
      value: entries[index]
    };
  }
  if (current.index === null) return null;
  const index = current.index + 1;
  if (index >= entries.length) return { state: { index: null, draft: "" }, value: current.draft };
  return { state: { ...current, index }, value: entries[index] };
}
