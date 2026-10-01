import { FOCUS_COMPOSER_EVENT, INSERT_TERMINAL_SELECTION_EVENT, type TerminalSelectionDetail } from "../chat/composer/composer-events";
import type { BrowserPickedElement } from "./browser-protocol";

/** 单个字段写入上下文时的最大字符数。 */
const MAX_FIELD_CHARS = 2_000;

/**
 * 截断过长字段，保留开头。
 *
 * @param value 原文
 * @returns 截断后的文本
 */
function clip(value: string): string {
  return value.length > MAX_FIELD_CHARS ? `${value.slice(0, MAX_FIELD_CHARS)}…` : value;
}

/**
 * 把选中的网页元素整理成交给模型的 Markdown 说明。
 *
 * 字段顺序先给定位信息（页面、选择器），再给内容与样式，方便模型直接修改对应代码。
 *
 * @param element 元素信息
 * @returns Markdown 文本
 */
export function formatPickedElement(element: BrowserPickedElement): string {
  const lines = [
    `Page: ${element.pageTitle || element.pageUrl}`,
    `URL: ${element.pageUrl}`,
    `Element: <${element.tagName}>${element.role ? ` role=${element.role}` : ""}`
  ];
  if (element.accessibleName) lines.push(`Name: ${element.accessibleName}`);
  if (element.selector) lines.push(`Selector: ${element.selector}`);
  if (element.xpath) lines.push(`XPath: ${element.xpath}`);
  if (element.rect) {
    const { x, y, width, height } = element.rect;
    lines.push(`Box: ${width}x${height} at (${x}, ${y})`);
  }
  if (element.style) {
    const style = Object.entries(element.style).map(([key, value]) => `${key}: ${value}`).join("; ");
    if (style) lines.push(`Style: ${style}`);
  }
  if (element.attributes && Object.keys(element.attributes).length > 0) {
    lines.push(`Attributes: ${Object.entries(element.attributes).map(([key, value]) => `${key}="${value}"`).join(" ")}`);
  }
  if (element.text) lines.push(`Text: ${clip(element.text)}`);
  if (element.nearbyText) lines.push(`Nearby text: ${clip(element.nearbyText)}`);
  if (element.htmlExcerpt) lines.push("HTML:", "```html", clip(element.htmlExcerpt), "```");
  return lines.join("\n");
}

/**
 * 选中元素的短标签，显示在输入框原子上。
 *
 * @param element 元素信息
 * @returns 例如 `Browser · button#buy`
 */
export function pickedElementLabel(element: BrowserPickedElement): string {
  const id = element.attributes?.id ? `#${element.attributes.id}` : "";
  return `Browser · ${element.tagName}${id}`;
}

/**
 * 把选中的网页元素作为输入原子加入聊天输入框，并把焦点交给输入框。
 *
 * @param element 元素信息
 * @returns 无
 */
export function sendPickedElementToChat(element: BrowserPickedElement): void {
  window.dispatchEvent(new CustomEvent<TerminalSelectionDetail>(INSERT_TERMINAL_SELECTION_EVENT, {
    detail: { source: pickedElementLabel(element), content: formatPickedElement(element) }
  }));
  requestAnimationFrame(() => window.dispatchEvent(new Event(FOCUS_COMPOSER_EVENT)));
}
