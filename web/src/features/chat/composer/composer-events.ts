import { formatTerminalSelection } from "./composer-atom-token";

export const INSERT_TERMINAL_SELECTION_EVENT = "sai:insert-terminal-selection";
export const FOCUS_COMPOSER_EVENT = "sai:focus-composer";

export type TerminalSelectionDetail = {
  source: string;
  content: string;
};

/**
 * 将终端选区追加到当前输入，并保持与相邻文本之间有一个空格。
 *
 * @param current 当前输入
 * @param detail 终端标题和选区内容
 * @returns 追加终端原子后的输入
 */
export function appendTerminalSelection(current: string, detail: TerminalSelectionDetail): string {
  const atom = formatTerminalSelection(detail.source, detail.content);
  if (!current) return `${atom} `;
  return `${current}${/\s$/u.test(current) ? "" : " "}${atom} `;
}
