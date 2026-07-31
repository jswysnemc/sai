export type MarkdownAstNode = {
  type: string;
  value?: string;
  lang?: string;
  children?: MarkdownAstNode[];
};

const SVG_MARKUP_PATTERN = /^\s*(?:<\?xml[\s\S]*?\?>\s*)?<svg(?:\s|>)[\s\S]*<\/svg>\s*$/iu;

/**
 * 判断文本是否为独立 SVG 文档。
 *
 * @param source 待检查文本
 * @returns 文本仅包含 SVG 根元素时返回 true
 */
export function isSvgMarkup(source: string): boolean {
  return SVG_MARKUP_PATTERN.test(source);
}

/**
 * 把 SVG 文本编码为浏览器图片上下文可加载的 data URL。
 *
 * @param source SVG 源码
 * @returns SVG 图片 URL，输入不是独立 SVG 时返回 null
 */
export function toSvgDataUrl(source: string): string | null {
  if (!isSvgMarkup(source)) return null;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(source.trim())}`;
}

/**
 * 递归把 Markdown 中的原始 SVG HTML 块转换为 svg 围栏代码节点。
 *
 * @param node Markdown 抽象语法树节点
 * @returns 无返回值，直接更新传入语法树
 */
function transformSvgHtmlBlocks(node: MarkdownAstNode): void {
  if (!node.children) return;
  for (let index = 0; index < node.children.length; index += 1) {
    const child = node.children[index];
    const svg = extractSvgMarkup(child);
    if (svg) {
      node.children[index] = { type: "code", lang: "svg", value: svg };
      continue;
    }
    transformSvgHtmlBlocks(child);
  }
}

/**
 * 从原始 HTML 节点或仅含 HTML 的段落节点提取完整 SVG。
 *
 * @param node 待检查 Markdown 节点
 * @returns 完整 SVG 源码，不满足条件时返回 null
 */
function extractSvgMarkup(node: MarkdownAstNode): string | null {
  if (node.type === "html" && node.value && isSvgMarkup(node.value)) return node.value.trim();
  if (node.type !== "paragraph" || !node.children?.length) return null;
  if (!node.children.every((child) => child.type === "html" && typeof child.value === "string")) return null;
  const source = node.children.map((child) => child.value).join("");
  return isSvgMarkup(source) ? source.trim() : null;
}

/**
 * 创建 ReactMarkdown 使用的原始 SVG 块转换插件。
 *
 * @returns Markdown 抽象语法树转换函数
 */
export function remarkSvgBlocks() {
  return (tree: MarkdownAstNode): void => transformSvgHtmlBlocks(tree);
}
