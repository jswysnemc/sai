import { BookOpen, FileText, SquareTerminal, Target, type LucideIcon } from "lucide-react";
import { createRoot, type Root } from "react-dom/client";
import { parseComposerAtoms, type ComposerAtomSegment } from "./composer-atom-token";

const ATOM_ATTRIBUTE = "data-composer-atom";
const iconRoots = new WeakMap<HTMLElement, Root>();

export type EditorTextSelection = {
  start: number;
  end: number;
};

/**
 * 使用普通文本和不可编辑原子重建输入区内容。
 *
 * @param editor 输入区根元素
 * @param value 后端文本格式的输入内容
 * @returns 无返回值
 */
export function renderComposerAtomEditor(editor: HTMLElement, value: string): void {
  unmountIcons(editor);
  const fragment = document.createDocumentFragment();
  for (const segment of parseComposerAtoms(value)) {
    if (segment.type === "text") {
      fragment.append(document.createTextNode(segment.value));
      continue;
    }
    fragment.append(createAtom(segment));
  }
  editor.replaceChildren(fragment);
}

/**
 * 将输入区 DOM 序列化为后端可理解的纯文本。
 *
 * @param editor 输入区根元素或克隆片段
 * @returns 包含全部输入原子的纯文本协议
 */
export function serializeComposerAtomEditor(editor: Node): string {
  let value = "";
  for (const child of editor.childNodes) {
    if (child.nodeType === Node.TEXT_NODE) {
      value += child.nodeValue ?? "";
      continue;
    }
    if (!(child instanceof HTMLElement)) {
      value += serializeComposerAtomEditor(child);
      continue;
    }
    const atom = child.getAttribute(ATOM_ATTRIBUTE);
    if (atom !== null) {
      value += atom;
      continue;
    }
    if (child.tagName === "BR") {
      value += "\n";
      continue;
    }
    value += serializeComposerAtomEditor(child);
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
 * 选择点击到的完整输入原子，避免光标进入原子正文。
 *
 * @param editor 输入区根元素
 * @param target 指针事件目标
 * @returns 是否选择了输入原子
 */
export function selectComposerAtom(editor: HTMLElement, target: EventTarget | null): boolean {
  const atom = target instanceof Element ? target.closest<HTMLElement>(`[${ATOM_ATTRIBUTE}]`) : null;
  if (!atom || !editor.contains(atom)) return false;
  const selection = window.getSelection();
  if (!selection) return false;
  const range = document.createRange();
  range.selectNode(atom);
  selection.removeAllRanges();
  selection.addRange(range);
  return true;
}

/**
 * 删除光标相邻的完整输入原子。
 *
 * @param editor 输入区根元素
 * @param direction 删除方向
 * @returns 是否删除了输入原子
 */
export function deleteAdjacentComposerAtom(editor: HTMLElement, direction: "backward" | "forward"): boolean {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0 || !selection.isCollapsed) return false;
  const range = selection.getRangeAt(0);
  if (!editor.contains(range.startContainer)) return false;
  const atom = adjacentAtom(range.startContainer, range.startOffset, direction, editor);
  if (!atom) return false;
  unmountIcon(atom);
  const deletion = document.createRange();
  deletion.selectNode(atom);
  deletion.deleteContents();
  deletion.collapse(direction === "backward");
  selection.removeAllRanges();
  selection.addRange(deletion);
  editor.normalize();
  return true;
}

/**
 * 在当前选区插入纯文本并保持 DOM 不含外部富文本节点。
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

/** 根据片段类型创建带 Lucide 图标的不可编辑原子节点。 */
function createAtom(segment: Exclude<ComposerAtomSegment, { type: "text" }>): HTMLElement {
  const atom = document.createElement("span");
  const presentation = atomPresentation(segment);
  atom.className = `composer-atom composer-${segment.type}-mention`;
  atom.contentEditable = "false";
  atom.dataset.composerAtom = segment.value;
  atom.dataset.atomKind = segment.type;
  atom.title = presentation.title;
  if (segment.type === "terminal") atom.dataset.preview = segment.content;

  const icon = document.createElement("span");
  icon.className = "composer-atom-icon";
  icon.dataset.composerAtomIcon = "true";
  const label = document.createElement("span");
  label.className = "composer-atom-label";
  label.textContent = presentation.label;
  atom.append(icon, label);

  const root = createRoot(icon);
  iconRoots.set(icon, root);
  root.render(<presentation.Icon size={12} aria-hidden="true" />);
  return atom;
}

/** 返回不同输入原子的图标、标签和悬停说明。 */
function atomPresentation(segment: Exclude<ComposerAtomSegment, { type: "text" }>): { Icon: LucideIcon; label: string; title: string } {
  if (segment.type === "file") return { Icon: FileText, label: segment.path, title: segment.path };
  if (segment.type === "skill") return { Icon: BookOpen, label: `/${segment.name}`, title: `Skill: ${segment.name}` };
  if (segment.type === "goal") return { Icon: Target, label: "/goal", title: "Use the remaining input as the session goal" };
  const lines = segment.content.split(/\r?\n/u).length;
  return {
    Icon: SquareTerminal,
    label: `${segment.source || "Terminal"} · ${lines} lines`,
    title: segment.content
  };
}

/** 卸载编辑器内全部图标根节点。 */
function unmountIcons(editor: HTMLElement): void {
  editor.querySelectorAll<HTMLElement>("[data-composer-atom-icon]").forEach(unmountIcon);
}

/** 卸载指定原子或图标节点中的 React 图标根。 */
function unmountIcon(node: HTMLElement): void {
  const icon = node.hasAttribute("data-composer-atom-icon")
    ? node
    : node.querySelector<HTMLElement>("[data-composer-atom-icon]");
  if (!icon) return;
  iconRoots.get(icon)?.unmount();
  iconRoots.delete(icon);
}

/** 计算 DOM 点之前的序列化文本长度。 */
function textOffsetAtPoint(editor: HTMLElement, container: Node, offset: number): number {
  const range = document.createRange();
  range.selectNodeContents(editor);
  range.setEnd(container, offset);
  return serializeComposerAtomEditor(range.cloneContents()).length;
}

/** 将纯文本偏移映射到可设置选区的 DOM 点。 */
function domPointAtTextOffset(editor: HTMLElement, requestedOffset: number): { node: Node; offset: number } {
  let remaining = Math.max(0, requestedOffset);
  for (let index = 0; index < editor.childNodes.length; index += 1) {
    const child = editor.childNodes[index];
    const length = serializeComposerAtomEditor(child).length || child.textContent?.length || 0;
    if (child.nodeType === Node.TEXT_NODE && remaining <= length) return { node: child, offset: remaining };
    if (child instanceof HTMLElement && child.hasAttribute(ATOM_ATTRIBUTE) && remaining <= length) {
      return remaining < length / 2 ? { node: editor, offset: index } : { node: editor, offset: index + 1 };
    }
    if (remaining <= length) return { node: child, offset: Math.min(remaining, child.childNodes.length) };
    remaining -= length;
  }
  return { node: editor, offset: editor.childNodes.length };
}

/** 查找折叠光标前后紧邻的输入原子节点。 */
function adjacentAtom(container: Node, offset: number, direction: "backward" | "forward", editor: HTMLElement): HTMLElement | null {
  let candidate: Node | null = null;
  if (container === editor) {
    candidate = editor.childNodes[direction === "backward" ? offset - 1 : offset] ?? null;
  } else if (container.nodeType === Node.TEXT_NODE) {
    const textLength = container.nodeValue?.length ?? 0;
    if (direction === "backward" && offset === 0) candidate = container.previousSibling;
    if (direction === "forward" && offset === textLength) candidate = container.nextSibling;
  }
  return candidate instanceof HTMLElement && candidate.hasAttribute(ATOM_ATTRIBUTE) ? candidate : null;
}
