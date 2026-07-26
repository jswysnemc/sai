/**
 * 表格单元格的轻量内联格式渲染。
 *
 * 只处理单层的行内代码、粗体、斜体、删除线与链接文字，
 * 不做嵌套解析：单元格里的嵌套写法极少，为其引入完整解析器不值得。
 * 全部通过 textContent 建 DOM，不经过 innerHTML。
 */

const INLINE_PATTERN =
  /`([^`]+)`|\*\*([^*]+)\*\*|__([^_]+)__|\*([^*\s](?:[^*]*[^*\s])?)\*|_([^_\s](?:[^_]*[^_\s])?)_|~~([^~]+)~~|\[([^\]]+)\]\([^)\s]+(?:\s+"[^"]*")?\)/g;

/**
 * 把带内联标记的文本渲染进给定元素。
 *
 * 参数:
 * - `parent`: 目标元素
 * - `text`: 单元格原文
 *
 * 返回:
 * - 无；结果以子节点形式追加到 parent
 */
export function appendInlineMarkdown(parent: HTMLElement, text: string): void {
  let last = 0;
  for (const match of text.matchAll(INLINE_PATTERN)) {
    const index = match.index ?? 0;
    if (index > last) {
      parent.appendChild(document.createTextNode(text.slice(last, index)));
    }
    parent.appendChild(inlineElement(match));
    last = index + match[0].length;
  }
  if (last < text.length) {
    parent.appendChild(document.createTextNode(text.slice(last)));
  }
}

/**
 * 把一个正则命中转换为对应的行内元素。
 *
 * 参数:
 * - `match`: INLINE_PATTERN 的命中结果
 *
 * 返回:
 * - 承载该片段的行内元素
 */
function inlineElement(match: RegExpMatchArray): HTMLElement {
  const [, code, bold, boldAlt, italic, italicAlt, strike, linkLabel] = match;
  if (code !== undefined) {
    return textElement("code", code, "cm-md-inline-code");
  }
  if (bold !== undefined || boldAlt !== undefined) {
    return textElement("strong", bold ?? boldAlt ?? "");
  }
  if (italic !== undefined || italicAlt !== undefined) {
    return textElement("em", italic ?? italicAlt ?? "");
  }
  if (strike !== undefined) {
    return textElement("s", strike);
  }
  return textElement("span", linkLabel ?? "", "cm-md-link");
}

/**
 * 创建带文本的元素。
 *
 * 参数:
 * - `tag`: 元素标签
 * - `text`: 文本内容
 * - `className`: 可选样式类
 *
 * 返回:
 * - 新建元素
 */
function textElement(tag: string, text: string, className?: string): HTMLElement {
  const element = document.createElement(tag);
  if (className) element.className = className;
  element.textContent = text;
  return element;
}
