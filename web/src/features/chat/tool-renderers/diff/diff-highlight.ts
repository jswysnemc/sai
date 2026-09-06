import { highlightSource, splitHighlightedLines } from "../../syntax-highlighter";
import type { DiffLine, DiffSegment } from "./diff-model";

export type DiffLineHighlight = { old?: string; new?: string };

/**
 * 分别着色修改前后的代码，并叠加字符差异，保留跨行注释和字符串的语法状态。
 * @param lines 已解析的补丁行
 * @param language 文件语言或扩展名
 * @returns 每行在旧文件、新文件中的安全着色标记
 */
export function highlightDiffLines(lines: DiffLine[], language?: string): Map<DiffLine, DiffLineHighlight> {
  const result = new Map<DiffLine, DiffLineHighlight>();
  let group: DiffLine[] = [];

  /**
   * 着色一段连续代码，避免跨越补丁中未提供的内容推断语法状态。
   * @returns 无返回值
   */
  const flush = () => {
    if (!group.length) return;
    for (const side of ["old", "new"] as const) {
      const sideLines = group.filter((line) => line.kind !== (side === "old" ? "added" : "removed"));
      if (!sideLines.length) continue;
      const html = highlightSource(sideLines.map((line) => line.text).join("\n"), language).value;
      const highlighted = splitHighlightedLines(html);
      sideLines.forEach((line, index) => {
        const pair = result.get(line) ?? {};
        pair[side] = markChangedText(highlighted[index] ?? "", line.segments);
        result.set(line, pair);
      });
    }
    group = [];
  };

  // 1. 【差异审阅】【代码着色】区块边界两侧独立解析，普通换行保留语法状态
  for (const line of lines) {
    if (line.kind === "hunk" || line.kind === "no-newline") flush();
    else group.push(line);
  }
  flush();
  return result;
}

/**
 * 在安全语法标记内部添加字符变更背景，不破坏标签或 HTML 实体。
 * @param html 高亮器输出的单行安全 HTML
 * @param segments 该行的字符差异片段
 * @returns 同时保留语法颜色与字符变更背景的 HTML
 */
export function markChangedText(html: string, segments?: DiffSegment[]): string {
  if (!segments?.some((segment) => segment.changed)) return html;
  let offset = 0;
  const ranges = segments.flatMap((segment) => {
    const start = offset;
    offset += segment.text.length;
    return segment.changed && offset > start ? [{ start, end: offset }] : [];
  });
  let position = 0;
  return (html.match(/<[^>]+>|&(?:#x[\da-f]+|#\d+|\w+);|[^<&]+|[<&]/giu) ?? []).map((token) => {
    if (token.startsWith("<")) return token;
    const entity = token.startsWith("&") && token.endsWith(";");
    const length = entity ? entityLength(token) : token.length;
    const start = position;
    position += length;
    if (entity) {
      return ranges.some((range) => range.start < position && range.end > start)
        ? `<mark class="diff-inline">${token}</mark>` : token;
    }
    let cursor = 0;
    let output = "";
    for (const range of ranges) {
      const from = Math.max(0, range.start - start);
      const to = Math.min(length, range.end - start);
      if (from >= to) continue;
      output += token.slice(cursor, from);
      output += `<mark class="diff-inline">${token.slice(from, to)}</mark>`;
      cursor = to;
    }
    return output + token.slice(cursor);
  }).join("");
}

/**
 * 计算 HTML 实体对应的 UTF-16 长度，与差异片段的字符串偏移保持一致。
 * @param entity 高亮器输出的 HTML 实体
 * @returns 原文占用的 UTF-16 单元数
 */
function entityLength(entity: string): number {
  const match = /^&#(x[\da-f]+|\d+);$/iu.exec(entity);
  if (!match) return 1;
  const hexadecimal = match[1].toLowerCase().startsWith("x");
  const point = Number.parseInt(hexadecimal ? match[1].slice(1) : match[1], hexadecimal ? 16 : 10);
  return point > 0xffff ? 2 : 1;
}
