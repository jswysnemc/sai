import { formatTerminalSelection } from "./composer-atom-token";

export const INSERT_TERMINAL_SELECTION_EVENT = "sai:insert-terminal-selection";
export const INSERT_COMPOSER_TEXT_EVENT = "sai:insert-composer-text";
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

export type ComposerTextDetail = {
  content: string;
  quote?: boolean;
};

/**
 * 把选区或摘要追加到输入区。
 *
 * @param current 当前草稿
 * @param detail 纯文本及是否引用
 * @returns 追加后的草稿
 */
export function appendComposerText(current: string, detail: ComposerTextDetail): string {
  const body = detail.quote
    ? detail.content.split("\n").map((line) => `> ${line}`).join("\n")
    : detail.content;
  if (!current) return body;
  return `${current}${current.endsWith("\n") ? "" : "\n"}${body}`;
}

/**
 * 把文本写入输入区并聚焦。
 *
 * @param content 要写入的文本
 * @param quote 是否按引用块写入
 * @returns 无
 */
export function dispatchComposerText(content: string, quote = false): void {
  const trimmed = content.trim();
  if (!trimmed) return;
  window.dispatchEvent(new CustomEvent<ComposerTextDetail>(INSERT_COMPOSER_TEXT_EVENT, {
    detail: { content: trimmed, quote }
  }));
  requestAnimationFrame(() => window.dispatchEvent(new Event(FOCUS_COMPOSER_EVENT)));
}
