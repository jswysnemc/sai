import { parseFileMentions } from "./file-mention-token";

const MENTION_ATTRIBUTE = "data-file-mention";

export type EditorTextSelection = {
  start: number;
  end: number;
};

/**
 * 使用普通文本和不可编辑 token 重建输入区内容。
 *
 * @param editor 输入区根元素
 * @param value 后端文本格式的输入内容
 * @returns 无返回值
 */
export function renderFileMentionEditor(editor: HTMLElement, value: string): void {
  const fragment = document.createDocumentFragment();
  for (const segment of parseFileMentions(value)) {
    if (segment.type === "text") {
      fragment.append(document.createTextNode(segment.value));
      continue;
    }
    const token = document.createElement("span");
    token.className = "composer-file-mention";
    token.contentEditable = "false";
    token.dataset.fileMention = segment.value;
    token.dataset.filePath = segment.path;
    token.textContent = segment.value;
    fragment.append(token);
  }
  editor.replaceChildren(fragment);
}

/**
 * 将输入区 DOM 序列化为后端可理解的纯文本。
 *
 * @param editor 输入区根元素或克隆片段
 * @returns 包含 @ 文件引用的纯文本
 */
export function serializeFileMentionEditor(editor: Node): string {
  let value = "";
  for (const child of editor.childNodes) {
    if (child.nodeType === Node.TEXT_NODE) {
      value += child.nodeValue ?? "";
      continue;
    }
    if (!(child instanceof HTMLElement)) {
      value += serializeFileMentionEditor(child);
      continue;
    }
    const mention = child.getAttribute(MENTION_ATTRIBUTE);
    if (mention !== null) {
      value += mention;
      continue;
    }
    if (child.tagName === "BR") {
      value += "\n";
      continue;
    }
    value += serializeFileMentionEditor(child);
    if ((child.tagName === "DIV" || child.tagName === "P") && child.nextSibling) value += "\n";
  }
  return value;
}

/**
 * 将浏览器选区转换为纯文本中的起止偏移。
 *
 * @param editor 输入区根元素
 * @returns 选区不在输入区时返回 null
 */
export function readEditorTextSelection(editor: HTMLElement): EditorTextSelection | null {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0) return null;
  const range = selection.getRangeAt(0);
  if (!editor.contains(range.startContainer) || !editor.contains(range.endContainer)) return null;
  const start = textOffsetAtPoint(editor, range.startContainer, range.startOffset);
  const end = textOffsetAtPoint(editor, range.endContainer, range.endOffset);
  return { start: Math.min(start, end), end: Math.max(start, end) };
}

/**
 * 按纯文本偏移恢复输入区选区。
 *
 * @param editor 输入区根元素
 * @param start 选区起点
 * @param end 选区终点，省略时为折叠选区
 * @returns 无返回值
 */
export function setEditorTextSelection(editor: HTMLElement, start: number, end = start): void {
  const selection = window.getSelection();
  if (!selection) return;
  const range = document.createRange();
  const startPoint = domPointAtTextOffset(editor, start);
  const endPoint = domPointAtTextOffset(editor, end);
  range.setStart(startPoint.node, startPoint.offset);
  range.setEnd(endPoint.node, endPoint.offset);
  selection.removeAllRanges();
  selection.addRange(range);
}

/**
 * 选择点击到的完整文件引用 token。
 *
 * @param editor 输入区根元素
 * @param target 指针事件目标
 * @returns 是否选择了文件引用
 */
export function selectFileMentionToken(editor: HTMLElement, target: EventTarget | null): boolean {
  const token = target instanceof Element ? target.closest<HTMLElement>(`[${MENTION_ATTRIBUTE}]`) : null;
  if (!token || !editor.contains(token)) return false;
  const selection = window.getSelection();
  if (!selection) return false;
  const range = document.createRange();
  range.selectNode(token);
  selection.removeAllRanges();
  selection.addRange(range);
  return true;
}

/**
 * 删除光标相邻的完整文件引用 token。
 *
 * @param editor 输入区根元素
 * @param direction 删除方向
 * @returns 是否删除了文件引用
 */
export function deleteAdjacentFileMention(editor: HTMLElement, direction: "backward" | "forward"): boolean {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0 || !selection.isCollapsed) return false;
  const range = selection.getRangeAt(0);
  if (!editor.contains(range.startContainer)) return false;
  const token = adjacentMention(range.startContainer, range.startOffset, direction, editor);
  if (!token) return false;
  const deletion = document.createRange();
  deletion.selectNode(token);
  deletion.deleteContents();
  deletion.collapse(direction === "backward");
  selection.removeAllRanges();
  selection.addRange(deletion);
  editor.normalize();
  return true;
}

/**
 * 在当前选区插入纯文本并保持 DOM 不含富文本节点。
 *
 * @param editor 输入区根元素
 * @param text 要插入的文本
 * @returns 是否完成插入
 */
export function insertEditorPlainText(editor: HTMLElement, text: string): boolean {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0) return false;
  const range = selection.getRangeAt(0);
  if (!editor.contains(range.startContainer)) return false;
  range.deleteContents();
  const node = document.createTextNode(text);
  range.insertNode(node);
  range.setStartAfter(node);
  range.collapse(true);
  selection.removeAllRanges();
  selection.addRange(range);
  editor.normalize();
  return true;
}

/**
 * 计算 DOM 点之前的序列化文本长度。
 *
 * @param editor 输入区根元素
 * @param container 选区容器
 * @param offset 容器内偏移
 * @returns 纯文本偏移
 */
function textOffsetAtPoint(editor: HTMLElement, container: Node, offset: number): number {
  const range = document.createRange();
  range.selectNodeContents(editor);
  range.setEnd(container, offset);
  return serializeFileMentionEditor(range.cloneContents()).length;
}

/**
 * 将纯文本偏移映射到可设置选区的 DOM 点。
 *
 * @param editor 输入区根元素
 * @param requestedOffset 请求偏移
 * @returns DOM 节点与节点内偏移
 */
function domPointAtTextOffset(editor: HTMLElement, requestedOffset: number): { node: Node; offset: number } {
  let remaining = Math.max(0, requestedOffset);
  for (let index = 0; index < editor.childNodes.length; index += 1) {
    const child = editor.childNodes[index];
    const length = serializeFileMentionEditor(child).length || child.textContent?.length || 0;
    if (child.nodeType === Node.TEXT_NODE && remaining <= length) return { node: child, offset: remaining };
    if (child instanceof HTMLElement && child.hasAttribute(MENTION_ATTRIBUTE) && remaining <= length) {
      return remaining < length / 2 ? { node: editor, offset: index } : { node: editor, offset: index + 1 };
    }
    if (remaining <= length) return { node: child, offset: Math.min(remaining, child.childNodes.length) };
    remaining -= length;
  }
  return { node: editor, offset: editor.childNodes.length };
}

/**
 * 查找折叠光标前后紧邻的文件引用节点。
 *
 * @param container 光标容器
 * @param offset 光标偏移
 * @param direction 查找方向
 * @param editor 输入区根元素
 * @returns 相邻 token，不存在时返回 null
 */
function adjacentMention(container: Node, offset: number, direction: "backward" | "forward", editor: HTMLElement): HTMLElement | null {
  let candidate: Node | null = null;
  if (container === editor) {
    candidate = editor.childNodes[direction === "backward" ? offset - 1 : offset] ?? null;
  } else if (container.nodeType === Node.TEXT_NODE) {
    const textLength = container.nodeValue?.length ?? 0;
    if (direction === "backward" && offset === 0) candidate = container.previousSibling;
    if (direction === "forward" && offset === textLength) candidate = container.nextSibling;
  }
  return candidate instanceof HTMLElement && candidate.hasAttribute(MENTION_ATTRIBUTE) ? candidate : null;
}
