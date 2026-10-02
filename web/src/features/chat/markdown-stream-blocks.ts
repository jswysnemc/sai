import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";

const parser = unified().use(remarkParse).use(remarkGfm).use(remarkMath);
export type MarkdownStreamBlock = { offset: number; source: string };
export type MarkdownStreamState = { source: string; blocks: MarkdownStreamBlock[]; tailOffset: number };

/**
 * 【正文渲染】【增量分块】复用已完成的顶层块，仅重新解析末尾两个可变语法块。
 * @param source 当前完整 Markdown
 * @param previous 上一次分块结果；非追加编辑时自动失效
 * @returns 保留原始文本、稳定偏移键与可变尾部的分块结果
 */
export function splitMarkdownStream(source: string, previous?: MarkdownStreamState): MarkdownStreamState {
  if (source === previous?.source) return previous;
  // 1. 引用定义和脚注可以影响任意早期段落，出现定义标记时使用完整文档解析
  if (source.includes("]:")) return { source, blocks: [{ offset: 0, source }], tailOffset: 0 };
  const append = previous && source.startsWith(previous.source);
  const offset = append ? previous.tailOffset : 0;
  const stable = append ? previous.blocks.filter((block) => block.offset < offset) : [];
  const tail = source.slice(offset);
  const tree = parser.parse(tail);
  // 2. 使用与正文一致的 CommonMark/GFM/数学语法识别边界，不按空行切断列表、围栏或公式
  const stableCount = Math.max(0, tree.children.length - 2);
  let start = 0;
  for (let index = 0; index < stableCount; index += 1) {
    const end = tree.children[index + 1].position?.start.offset;
    if (end === undefined) break;
    stable.push({ offset: offset + start, source: tail.slice(start, end) });
    start = end;
  }
  const tailOffset = offset + start;
  stable.push({ offset: tailOffset, source: source.slice(tailOffset) });
  return { source, blocks: stable, tailOffset };
}
